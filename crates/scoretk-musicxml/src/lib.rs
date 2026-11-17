// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Lecture MusicXML vers le modèle de `scoretk_core`.
//!
//! Portage de l'import MusicXML de MuseScore Studio
//! (`src/importexport/musicxml/internal/import/`). Le lecteur accepte un
//! document `score-partwise` nu (`.musicxml`, `.xml`) ou une archive `.mxl`,
//! et renvoie un [`scoretk_core::model::Score`], le modèle pivot commun aux
//! formats de scoretk. `read_musicxml` accepte les deux ; `read_mxl` exige une
//! archive et porte `importCompressedMusicXml`.
//!
//! # Portée
//!
//! Cette première version couvre le **noyau musical** : parties et portées,
//! mesures, armures, indications de mesure, clefs, accords, silences, durées,
//! voix (`backup`/`forward`), n-olets, liaisons de prolongation, paroles et
//! reprises. Les directions (nuances, soufflets, tempos), les articulations,
//! les spanners et les harmonies ne sont pas encore lus ; le détail et les
//! écarts mesurés sont dans `docs/agents/musicxml-import.md`.
//!
//! # Exemple
//!
//! ```
//! use scoretk_musicxml::read_musicxml;
//!
//! let musicxml = br#"<score-partwise version="4.0">
//!   <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
//!   <part id="P1"><measure number="1">
//!     <attributes><divisions>1</divisions>
//!       <key><fifths>0</fifths></key>
//!       <time><beats>4</beats><beat-type>4</beat-type></time>
//!       <clef><sign>G</sign><line>2</line></clef>
//!     </attributes>
//!     <note><pitch><step>C</step><octave>4</octave></pitch>
//!       <duration>4</duration><voice>1</voice><type>whole</type></note>
//!   </measure></part>
//! </score-partwise>"#;
//!
//! let score = read_musicxml(musicxml)?;
//! assert_eq!(score.parts.len(), 1);
//! assert_eq!(score.measures.len(), 1);
//! # Ok::<(), scoretk_musicxml::Error>(())
//! ```
//!
//! # Erreurs
//!
//! Renvoie une [`Error`] si le conteneur `.mxl` est illisible, si le XML est
//! mal formé ou si la racine n'est pas `score-partwise`.

mod barline;
mod container;
mod credits;
mod direction;
mod error;
mod harmony;
mod mxl;
mod notations;
mod noteduration;
mod pass1;
mod pass2;
mod xml;

pub use error::{Error, Result};

use scoretk_core::model::Score;

/// Lit un document MusicXML `partwise` (`.musicxml`, `.xml` ou `.mxl`).
///
/// Tout se passe en mémoire : aucun fichier n'est ouvert, aucune horloge n'est
/// lue. Le résultat est un [`Score`] du modèle de `scoretk_core`, exportable en
/// MusicXML ou convertible vers un autre format.
pub fn read_musicxml(bytes: &[u8]) -> Result<Score> {
    let document = container::extract(bytes)?;
    parse_document(&document)
}

/// Lit un fichier MusicXML compressé (`.mxl`) et renvoie le modèle pivot.
///
/// Équivalent de `importCompressedMusicXml` de MuseScore : le document est
/// extrait de l'archive (voir `mxl.rs`), puis passé au lecteur commun. Un
/// document MusicXML nu est refusé ; c'est le rôle de [`read_musicxml`].
pub fn read_mxl(bytes: &[u8]) -> Result<Score> {
    mxl::read_mxl(bytes)
}

/// Passe le document MusicXML extrait au lecteur (`importMusicXmlfromBuffer`).
pub(crate) fn parse_document(document: &[u8]) -> Result<Score> {
    let root = xml::parse(document)?;
    if root.name == "score-timewise" {
        return Err(Error::UnknownRoot(root.name));
    }
    let analysis = pass1::analyse(&root)?;
    let mut score = pass2::build(&root, &analysis)?;
    // `Measure::checkMeasure` : les trous des voix sont comblés par des
    // silences « gap », qui ne sont pas exportés mais font avancer le curseur
    // (origine des `<forward>`/`<backup>` scindés de MuseScore).
    scoretk_core::read::gap_rests::fill(&mut score);
    Ok(score)
}
