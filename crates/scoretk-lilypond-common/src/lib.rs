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

/// `time-sig` value whose subtype has no `\time` symbol in LilyPond 2.26
/// (Notation Reference §2.3.1): MuseScore's `AllaBreve` (cut common time).
pub fn time_sig_out_of_reach(value: &str) -> bool {
    value.contains("AllaBreve")
}

/// MuseScore-only style default with no LilyPond counterpart.
pub fn style_out_of_reach(value: &str) -> bool {
    value.starts_with("hide_empty_staves:")
}

/// Item kinds that LilyPond 2.26 can carry as *format* data (§5.3 of
/// `docs/agents/model-modularisation.md`).
///
/// Five kinds of the pivot inventory are deliberately absent, with evidence:
///
/// - `tbox-position` and `tbox-text` describe a MuseScore frame (measure index,
///   placement, font size, frame text). LilyPond has no frame object:
///   `\markup` attaches to a note or a context, not to a numbered MuseScore
///   frame (Notation Reference §6.1, §8.2).
/// - `credit-position` describes a credit's justification, vertical alignment,
///   page and frame height. LilyPond's `\header` fields have no per-field
///   position (Notation Reference §21.1.2); `\paper` only sets global page
///   geometry.
/// - `harmony` expects a `<harmony>` chord-symbol object at a tick. LilyPond
///   chord names live in a separate `ChordNames` context; `musicxml2ly`
///   writes MuseScore's text as `^\markup`, which another tool reads as text,
///   not as a chord symbol (`docs/agents/format-mapping/lilypond.md`).
/// - `system-text` distinguishes a system text from a staff text. LilyPond
///   `\markup` has no such distinction (Notation Reference §8.2).
/// - `instrument` expects a MuseScore instrument id (`strings.violin`).
///   LilyPond's `midiInstrument` only names a General MIDI program
///   (Notation Reference §24.5); the id has no carrier.
/// - `channel` expects the instrument's zero-based MIDI channel. LilyPond has
///   no per-instrument channel property (`midiChannelMapping` is a map, not a
///   channel number, §24.8).
///
/// Two more values are excluded individually, not whole kinds:
/// `time-sig` values with the `AllaBreve` subtype (§2.3.1 gives no cut-time
/// symbol for `\time`) and the MuseScore-only `hide_empty_staves` style
/// default (§11.4 of `docs/agents/model-modularisation.md`).
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
    "chord-line",
    "clef",
    "credit",
    "dynamic",
    "dynamic-velocity",
    "fermata",
    "figured-bass",
    "glissando",
    "grace",
    "grace-note",
    "hairpin",
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
