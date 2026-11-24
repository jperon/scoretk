// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage d'un sous-ensemble du langage LilyPond d'après `lily/parser.yy`,
// `lily/lexer.ll`, `lily/pitch.cc` et `scm/define-note-names.scm` de
// LilyPond 2.26.0, Copyright (C) 1998--2026 Han-Wen Nienhuys et al.,
// GPL-3.0-or-later ; réécriture en Rust en 2026.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! **scoretk-lilypond-import** : lecture d'un sous-ensemble de LilyPond vers le
//! modèle pivot. L'écriture est le fait de `scoretk-lilypond-export` ; les
//! tables communes (erreur, spirale des quintes, clefs) sont dans
//! `scoretk-lilypond-common`.
//!
//! LilyPond n'est pas intégrable : son parseur dépend de Guile Scheme. Ce
//! crate porte donc en Rust une partie de la grammaire (`lily/parser.yy`) et
//! du lexer (`lily/lexer.ll`) pour un sous-ensemble de notation, sur le modèle
//! de `python-ly` (`ly.music`, GPL-3.0-or-later) qui lit LilyPond sans Scheme.
//!
//! # Sous-ensemble couvert
//!
//! Notes, silences, accords, durées et points, `\relative`, `\fixed` et notation
//! absolue, `\transpose`, `\tuplet`/`\times`, liaisons `~`, `(`/`)`,
//! `\repeat volta`/`unfold` et `\alternative`, `\key`, `\time`, `\clef`,
//! `\partial`, `\bar`, `\language` (toutes les langues de
//! `define-note-names.scm`), `\include` des fichiers de langue et des fichiers
//! livrés sans musique, nuances, `\< \> \!`, articulations courantes,
//! `\grace`,
//! `\acciaccatura`, `\appoggiatura`, `R`/`s`, voix `<< \\ >>`, variables
//! (`nom = { … }` puis `\nom`), `\parallelMusic` (musique découpée aux
//! contrôles de mesure et répartie entre plusieurs variables), `\new Staff`/
//! `PianoStaff`/`ChoirStaff`/`Voice`/`Lyrics`, `\addlyrics`, `\lyricsto`,
//! `\lyricmode`, `\header`, `\tempo` et `\markup` simple.
//!
//! Tout ce qui sort de ce sous-ensemble (Scheme `#…`/`$…`, commandes inconnues)
//! est ignoré avec un avertissement collecté : la conversion ne doit pas
//! échouer sur une décoration. Les limites et les écarts mesurés sont dans
//! `docs/agents/lilypond.md`.
//!
//! Les `\include` sont étendus avant l'analyse, sur les jetons : les fichiers
//! de langue livrés par LilyPond valent `\language`, les fichiers livrés sans
//! musique sont ignorés sans avertissement, et les autres passent par le
//! rappel fourni à [`read_lilypond_with_includes`].
//!
//! Le crate ne fait aucune entrée/sortie : [`read_lilypond`] prend la source en
//! mémoire, ce qui le garde compilable pour `wasm32-unknown-unknown`, Android
//! et iOS.

mod ast;
mod builder;
mod events;
mod include;
mod lexer;
mod midi_instruments;
mod modes;
mod parallel;
mod parser;
mod partcombine;
mod pitch;
mod playback;
mod warnings;

pub use warnings::Warning;

pub use scoretk_lilypond_common::{Error, Result};

use scoretk_core::model::Score;

pub(crate) use events::Interpreter;

/// Lit une source LilyPond et renvoie le modèle pivot.
///
/// Les commandes hors du sous-ensemble sont ignorées ; pour les retrouver,
/// utiliser [`read_lilypond_reporting`]. Les `\include` de fichiers
/// ordinaires ne sont pas résolus : ils sont ignorés avec un avertissement.
pub fn read_lilypond(src: &str) -> Result<Score> {
    read_lilypond_reporting(src).map(|(score, _)| score)
}

/// Lit une source LilyPond et renvoie aussi les avertissements collectés.
pub fn read_lilypond_reporting(src: &str) -> Result<(Score, Vec<Warning>)> {
    read_lilypond_with_includes(src, &mut |_, _| None)
}

/// Lit une source LilyPond en résolvant ses `\include`.
///
/// `resolve(chemin_demandé, chemin_du_fichier_qui_inclut)` renvoie le texte du
/// fichier demandé, ou `None` s'il est introuvable ; le chemin du fichier qui
/// inclut vaut `None` au niveau supérieur, puis le chemin normalisé du fichier
/// inclus (relatif au fichier principal, `.` et `..` résolus). L'inclusion est
/// textuelle, comme dans `lily/lexer.ll` : les jetons du fichier inclus
/// remplacent la commande, au niveau supérieur comme au milieu de la musique
/// ou d'un `\score`. Les variables, `\language` et `\header` d'un fichier
/// inclus valent donc pour la suite.
///
/// Les fichiers de langue livrés par LilyPond (`italiano.ly`, `english.ly`…)
/// et les fichiers livrés sans musique (`articulate.ly`, `gregorian.ly`…)
/// restent traités sans appeler le résolveur. Les garde-fous sont la
/// profondeur maximale (32), la détection des cycles et l'avertissement pour
/// un `\include` d'expression Scheme.
///
/// La fonction ne fait aucune entrée/sortie : le résolveur peut lire une table
/// en mémoire, un disque ou rien du tout.
pub fn read_lilypond_with_includes(
    src: &str,
    resolve: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
) -> Result<(Score, Vec<Warning>)> {
    let mut warnings = warnings::Warnings::new();
    let document = parser::parse(src, resolve, &mut warnings);
    if document.score.is_none() {
        if warnings.music_ignored() {
            // Un `\include` ignoré peut porter le `\score` (gabarits
            // `satb.ly`, `ssaattbb.ly`) ; une grille d'accords ou de chiffres
            // peut aussi être la seule musique du fichier. Ce n'est pas une
            // erreur de syntaxe, mais une limite du sous-ensemble. On renvoie
            // une partition vide avec un avertissement plutôt que d'échouer.
            warnings.push(
                0,
                0,
                "aucune partition lisible : la musique est ignorée (include ou grille non jouée)",
            );
            let document = crate::ast::Document {
                score: Some(crate::ast::ScoreAst::default()),
                ..document
            };
            let score = builder::build(&document, &mut warnings);
            return Ok((score, warnings.into_vec()));
        }
        return Err(Error::Syntaxe(
            "aucune musique dans la source LilyPond".to_owned(),
        ));
    }
    let score = builder::build(&document, &mut warnings);
    Ok((score, warnings.into_vec()))
}
