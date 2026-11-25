// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les correspondances de notation (articulations, ornements, nuances, clés,
// armures) s'inspirent de `musicxml2ly` de LilyPond, Copyright (C) 2016--2026
// John Gourlay et Werner Lemberg, GPL-3.0-or-later ; le code ci-dessous est
// une réécriture en Rust, sans ligne copiée.
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

//! **scoretk-lilypond-common** : le peu que partagent la lecture et l'écriture
//! LilyPond.
//!
//! Cette crate ne contient aucune logique de conversion : ni lecture, ni
//! écriture d'un fichier, seulement des tables et des types communs aux deux
//! sens (règle de projet, cf. `docs/agents/developpement.md`).
//!
//! - [`error`] : l'erreur unique des deux sens ([`Error`], [`Result`]) ;
//! - [`pitch`] : la spirale des quintes (`tpc`) et ses conversions ;
//! - [`clefs`] : la table des clefs, dans les deux sens ;
//! - [`headers`] : les champs `\header` standards transportant les crédits.
//!
//! Tout le reste appartient à `scoretk-lilypond-import` (lecture `.ly` →
//! modèle) ou à `scoretk-lilypond-export` (modèle → `.ly`).
//!
//! Comme les deux autres, cette crate ne fait aucune entrée/sortie et compile
//! pour `wasm32-unknown-unknown`, Android et iOS.

pub mod clefs;
mod error;
pub mod headers;
pub mod midi_instruments;
pub mod noteheads;
pub mod pitch;

pub use error::{Error, Result};

/// Item kinds that LilyPond 2.26 can carry as *format* data (§5.3 of
/// `docs/agents/model-modularisation.md`).
///
/// Three kinds of the pivot inventory are deliberately absent, with evidence:
///
/// - `tbox-position` describes a MuseScore frame (measure index, vertical
///   placement, font size). LilyPond has no frame object: `\markup` attaches
///   to a note or a context, not to a numbered MuseScore frame (Notation
///   Reference §6.1, §8.2).
/// - `tremolo` distinguishes MuseScore's `Start`/`Stop`/`Unmeasured` spanner
///   kinds. LilyPond's `Tremolo` event (`Notation Reference §2.5.3`) has a
///   single strobe count; there is no start/stop pair to serialise.
/// - `note-head` uses the *exact* MuseScore `NoteHeadGroup` as its value.
///   LilyPond offers a fixed set of note-head styles (Notation Reference
///   appendix B.9); groups with no style (`Plus`, `WithX`, `HeavyCross`…)
///   have no carrier and cannot be reconstructed by any LilyPond tool.
///
/// The standard styles that do exist are still written and read; only the
/// exact-group identifier is outside the declaration. This is a declaration
/// about the format, not a relaxation of the comparison: the same inventory,
/// keys and oracle are used for every direction.
pub const LY_ITEM_KINDS: &[&str] = &[
    "arpeggio",
    "articulation",
    "barline",
    "barline-end",
    "barline-start",
    "beam",
    "breath",
    "channel",
    "chord-line",
    "clef",
    "credit",
    "credit-position",
    "dynamic",
    "dynamic-velocity",
    "fermata",
    "figured-bass",
    "glissando",
    "grace",
    "grace-note",
    "hairpin",
    "harmony",
    "instrument",
    "instrument-change",
    "jump",
    "key-sig",
    "line-break",
    "lyric",
    "marker",
    "measure",
    "measure-repeat",
    "midi-articulation",
    "multi-measure-rest",
    "note",
    "ornament",
    "ottava",
    "pan",
    "pedal",
    "program",
    "rehearsal-mark",
    "repeat-count",
    "rest",
    "section-break",
    "slur",
    "staff-text",
    "stem",
    "style",
    "style-internal",
    "system-text",
    "tbox-text",
    "tempo",
    "text-line",
    "tie",
    "time-sig",
    "title",
    "tremolo-bar",
    "trill",
    "tuplet",
    "tuplet-bracket",
    "tuplet-member",
    "volta",
    "volume",
];
