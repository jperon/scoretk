// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : constantes MEI 5.1 et valeurs `@type`
// propriétaires déclarées dans `meiconverter.h:64-89` de MuseScore Studio
// 4.7.4, Copyright (C) MuseScore Limited and others, réécrites en Rust en
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

//! Constants of the MEI 5.1 namespace and MuseScore's private `@type` values.
//!
//! The `mscore-*` values are extensions of MuseScore 4.7.4 and are not part of
//! MEI Basic: a strict MEI reader may ignore them.

/// Namespace of MEI 5.1 (`mei-all-5.1.rng` root).
pub const MEI_NAMESPACE: &str = "http://www.music-encoding.org/ns/mei";

/// Version written by MuseScore 4.7.4.
pub const MEI_VERSION: &str = "5.1+basic";

/// `@type="mscore-manual"` on `<pb>`/`<sb>` (`BREAK_TYPE`).
pub const BREAK_TYPE: &str = "mscore-manual";

/// `@type` prefix for beam values on `<chord>`/`<note>`/`<rest>`.
pub const BEAM_ELEMENT_TYPE: &str = "mscore-beam";

/// `@type` prefix for expression and play-technique annotations on `<dir>`.
pub const DIR_TYPE: &str = "mscore-";

/// `@type` prefix for a volta on `<ending>`.
pub const ENDING_REPEAT_TYPE: &str = "mscore-ending-";

/// `@type` prefix for chord symbols on `<harm>`.
pub const HARMONY_TYPE: &str = "mscore-";

/// `@type` prefix for jumps on `<repeatMark>`.
pub const JUMP_TYPE: &str = "mscore-jump-";

/// `@type` prefix for a measured repeat count on `<measure>`.
pub const MEASURE_REPEAT_TYPE: &str = "mscore-repeat-";

/// `@type` prefix for markers on `<repeatMark>`.
pub const MARKER_TYPE: &str = "mscore-marker-";

/// `@type` for a MuseScore text line written as a `<dir>` control event.
pub const TEXT_LINE_TYPE: &str = "mscore-text-line";

/// `@type` prefix carrying the exact ornament/breath variant on `<ornam>`
/// and `<breath>`/`<caesura>` (porteurs additionnels).
pub const ORNAMENT_TYPE: &str = "mscore-ornament-";
pub const BREATH_TYPE: &str = "mscore-breath-";

/// `@type` for a mid-score instrument change written as a `<dir>`.
pub const INSTRUMENT_CHANGE_TYPE: &str = "mscore-instrument-change";

/// `@label` prefix carrying the new instrument id on such a `<dir>`.
pub const INSTRUMENT_CHANGE_LABEL: &str = "mscore-instrument-change:";

/// `@type` prefix for a MuseScore text frame (`TBox`) written as a `<dir>`.
pub const TBOX_TYPE: &str = "mscore-tbox-";

/// `@label` prefix carrying a frame's measure, placement and font size.
pub const TBOX_FRAME_LABEL: &str = "mscore-tbox:";

/// `@type` prefix carrying the exact MuseScore grace type on `<graceGrp>`.
/// MEI's `@grace` only distinguishes `acc`/`unacc`/`unknown`, while the pivot
/// also records the written duration and an "after" variant.
pub const GRACE_TYPE: &str = "mscore-grace-";

/// `@label` prefix carrying a credit's page and frame height on `<rend>`.
/// MEI 5.1 has no element for a MuseScore `VBox`: `@n` keeps the credit order,
/// `@halign`/`@valign`/`@fontsize` are standard, and only the frame geometry
/// needs this documented MuseScore extension.
pub const CREDIT_FRAME_LABEL: &str = "mscore-frame-";

/// `@type` for a tempo inferred from its text.
pub const TEMPO_INFER_FROM_TEXT: &str = "mscore-infer-from-text";

/// `@type` prefix for chord lines on `<artic>`.
/// `@type` prefix for a MuseScore tremolo bar written as a `<dir>`.
pub const TREMOLO_BAR_TYPE: &str = "mscore-tremolo-bar-";

pub const CHORDLINE_TYPE: &str = "mscore-";

/// `@type` prefix for a MuseScore chord line (`falloff`, `doit`, …) on `<dir>`.
pub const CHORD_LINE_TYPE: &str = "mscore-chordline-";

/// `@type` prefix carrying the MuseScore instrument id on `<instrDef>`.
/// The numeric MIDI fields use standard `midi.*` attributes; the id itself
/// is a MuseScore extension (there is no open vocabulary for it in MEI 5.1:
/// `@midi.instrname` is restricted to `data.MIDINAMES`).
pub const INSTRUMENT_TYPE: &str = "mscore-instrument-";

/// `@type` prefix for ornament intervals.
pub const INTERVAL_ABOVE: &str = "mscore-above-";

/// `@type` prefix for ornament intervals below.
pub const INTERVAL_BELOW: &str = "mscore-below-";

/// SMuFL value of `@glyph.auth`.
pub const SMUFL_AUTH: &str = "smufl";

/// Sentinel for unset numeric attributes (`MEI_UNSET` in `meiconverter.h`).
pub const MEI_UNSET: i32 = -1;
