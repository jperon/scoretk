// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/style/styledef.cpp:174-266` of MuseScore Studio 4.7.4
// (tag v4.7.4), Copyright (C) MuseScore Limited and others, modified
// (rewritten in Rust) in 2026. Values from `leland_metadata.json`
// (`engravingDefaults`, SIL OFL 1.1) are marked SMuFL.
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

//! Default style values frozen as fixed-point i32 (1/1024 staff space).
//!
//! // Port of `styledef.cpp:174-266`: `clefLeftMargin` 0.75 sp,
//! `clefKeyRightMargin` 0.8 sp, `clefKeyDistance` 0.75 sp,
//! `clefTimesigDistance` 1.0 sp, `stemWidth` 0.10 sp, `stemLength` 3.5 sp,
//! `shortestStem` 2.5 sp, `staffLineWidth` 0.11 sp, `ledgerLineWidth`
//! 0.16 sp, `ledgerLineLength` 0.33 sp (a notehead-width multiplier),
//! `beamWidth` 0.5 sp, `dotMag` 1.0, `dotNoteDistance` 0.5 sp,
//! `dotRestDistance` 0.25 sp, `dotDotDistance` 0.65 sp.
//!
//! MuseScore reads these from `.mss` files at runtime
//! (`defaultstyle.cpp:37-41`); a generated constant table replaces that I/O
//! (plan §4). The values below are the 4.7.4 defaults, cross-checked against
//! Leland's SMuFL `engravingDefaults` (29 numeric entries in `scoretk-smufl`).

/// One staff space in fixed-point units.
pub const SP: i32 = 1024;
/// Half a staff space: one note step / line interval.
pub const HALF_SP: i32 = SP / 2;
/// Staff height, top line at y=0, bottom line at y=4 sp.
pub const STAFF_HEIGHT: i32 = 4 * SP;

pub const STAFF_LINE_WIDTH: i32 = 113; // styledef.cpp:237  (0.11 sp)
pub const STEM_WIDTH: i32 = 102; // styledef.cpp:217       (0.10 sp)
pub const STEM_LENGTH: i32 = 3584; // styledef.cpp:219     (3.5 sp)
pub const SHORTEST_STEM: i32 = 2560; // styledef.cpp:221   (2.5 sp)
pub const LEDGER_WIDTH: i32 = 164; // styledef.cpp:238 / SMuFL legerLineThickness
pub const LEDGER_EXTENSION: i32 = 338; // styledef.cpp:239 / SMuFL legerLineExtension 0.33 sp
pub const DOT_NOTE_DISTANCE: i32 = 512; // styledef.cpp:264 (0.5 sp)
pub const DOT_DOT_DISTANCE: i32 = 665; // styledef.cpp:266  (0.65 sp)
pub const CLEF_LEFT_MARGIN: i32 = 768; // styledef.cpp:174  (0.75 sp)
pub const CLEF_KEY_DISTANCE: i32 = 768; // styledef.cpp:181  (0.75 sp)
pub const KEY_TIME_DISTANCE: i32 = 1024; // styledef.cpp:182 (1.0 sp)
pub const THIN_BARLINE_WIDTH: i32 = 184; // SMuFL thinBarlineThickness 0.18 sp
pub const THICK_BARLINE_WIDTH: i32 = 563; // SMuFL thickBarlineThickness 0.55 sp
pub const BARLINE_SEPARATION: i32 = 379; // SMuFL barlineSeparation 0.37 sp
pub const ACCIDENTAL_DISTANCE: i32 = 128; // styledef.cpp:1438-1466, 0.125 sp

/// Width of a segment in the P1 fixed proportional spacing.
///
/// P2 replaces this with `HorizontalSpacing` (`horizontalspacing.cpp`).
pub const SEGMENT_WIDTH: i32 = 1536; // 1.5 sp
pub const MEASURE_GAP: i32 = 512; // 0.5 sp between measures
/// Extra width of the first measure (clef + key + time signature).
pub const FIRST_MEASURE_EXTRA: i32 = 4 * SP;
