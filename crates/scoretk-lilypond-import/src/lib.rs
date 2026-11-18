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
//! `\partial`, `\bar`, `\language` (néerlandais, anglais, allemand, italien,
//! français), nuances, `\< \> \!`, articulations courantes, `\grace`,
//! `\acciaccatura`, `\appoggiatura`, `R`/`s`, voix `<< \\ >>`, variables
//! (`nom = { … }` puis `\nom`), `\new Staff`/`PianoStaff`/`ChoirStaff`/`Voice`/
//! `Lyrics`, `\addlyrics`, `\lyricsto`, `\lyricmode`, `\header`, `\tempo` et
//! `\markup` simple.
//!
//! Tout ce qui sort de ce sous-ensemble (Scheme `#…`/`$…`, commandes inconnues)
//! est ignoré avec un avertissement collecté : la conversion ne doit pas
//! échouer sur une décoration. Les limites et les écarts mesurés sont dans
//! `docs/agents/lilypond.md`.
//!
//! Le crate ne fait aucune entrée/sortie : [`read_lilypond`] prend la source en
//! mémoire, ce qui le garde compilable pour `wasm32-unknown-unknown`, Android
//! et iOS.

mod ast;
mod builder;
mod events;
mod lexer;
mod parser;
mod pitch;
mod warnings;

pub use warnings::Warning;

pub use scoretk_lilypond_common::{Error, Result};

use scoretk_core::model::Score;

pub(crate) use events::Interpreter;

/// Lit une source LilyPond et renvoie le modèle pivot.
///
/// Les commandes hors du sous-ensemble sont ignorées ; pour les retrouver,
/// utiliser [`read_lilypond_reporting`].
pub fn read_lilypond(src: &str) -> Result<Score> {
    read_lilypond_reporting(src).map(|(score, _)| score)
}

/// Lit une source LilyPond et renvoie aussi les avertissements collectés.
pub fn read_lilypond_reporting(src: &str) -> Result<(Score, Vec<Warning>)> {
    let mut warnings = warnings::Warnings::new();
    let document = parser::parse(src, &mut warnings);
    if document.score.is_none() {
        return Err(Error::Syntaxe(
            "aucune musique dans la source LilyPond".to_owned(),
        ));
    }
    let score = builder::build(&document, &mut warnings);
    Ok((score, warnings.into_vec()))
}
