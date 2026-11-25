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

/// Valeur `tremolo` sans équivalent LilyPond : le buzz roll `Unmeasured`
/// (MuseScore `<tremolo type="unmeasured"/>`) n'a ni figure `c32` ni
/// `\repeat tremolo` (Notation Reference §2.5.3). Les trois autres genres sont
/// portés par `:8`/`:16`/`:32` et par `\repeat tremolo n { a b }`.
pub fn tremolo_out_of_reach(value: &str) -> bool {
    value == "Unmeasured"
}

/// Item kinds that LilyPond 2.26 can carry as *format* data (§5.3 of
/// `docs/agents/model-modularisation.md`).
///
/// One kind of the pivot inventory is deliberately absent, with evidence:
///
/// - `tbox-position` describes a MuseScore frame (measure index, vertical
///   placement, font size). LilyPond has no frame object: `\markup` attaches
///   to a note or a context, not to a numbered MuseScore frame (Notation
///   Reference §6.1, §8.2).
///
/// `tremolo` and `note-head` are declared again: LilyPond carries single-note
/// tremolos (`:8`, `:16`…) and two-note tremolos (`\repeat tremolo n`,
/// Notation Reference §2.5.3), and a fixed set of note-head styles
/// (`\tweak style #'cross`, `#'xcircle`, `#'harmonic`, `#'do`…, appendix B.9
/// and `select-head-glyph`). Only *individual values* with no LilyPond style —
/// listed in [`noteheads::out_of_reach`] — are excluded from the comparison,
/// never a whole kind. This is a declaration about the format, not a
/// relaxation of the comparison: the same inventory, keys and oracle are used
/// for every direction.
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
    "note-head",
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
    "tremolo",
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
