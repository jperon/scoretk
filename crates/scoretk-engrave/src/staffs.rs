// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/systemlayout.cpp:2477-2598`
// (`SystemLayout::layoutSystem`) and `:2748-2805` (`layoutInstrumentNames`)
// of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright (C) MuseScore Limited and
// others, modified (rewritten in Rust) in 2026: the `Staff`/`System` graph is
// replaced by plain per-staff numbers, copied once when the engraver is built.
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

//! Visible staves of the main score, with the geometry shared by a system.

use alloc::string::String;
use alloc::vec::Vec;

use scoretk_core::model::{Score, StaffBracket, StaffId};

use crate::layout::PageSpec;
use crate::style::STAFF_HEIGHT;

/// A staff that MuseScore engraves: hidden staves, hidden parts and excerpts
/// are not part of a `System` (`systemlayout.cpp:2477-2485`,
/// `pagelayout.cpp:661`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaffInfo {
    /// Index in `score.staves`: the track of every segment element is
    /// `index * VOICES + voice` (`ids.rs`, `Track::new`).
    pub index: usize,
    /// Index in `score.parts`, used for `akkoladeDistance` and instrument
    /// names (`systemlayout.cpp:2523-2530`, `:2748`).
    pub part: usize,
    /// Name on the first system.
    pub long_name: String,
    /// Name on the following systems.
    pub short_name: String,
    /// Brackets starting on this staff, with their span in staves.
    pub brackets: Vec<StaffBracket>,
    /// `Staff::barLineSpan()`: the bar line is shared with the next staff of
    /// the part (`part.rs`, `staff.h`).
    pub bar_line_span: bool,
}

impl StaffInfo {
    pub fn from_score(score: &Score) -> Vec<Self> {
        (0..score.staves.len())
            .filter(|&index| score.staff_shown(StaffId::from_raw(index as u32)))
            .map(|index| {
                let staff = &score.staves[index];
                let part = &score.parts[staff.part.index()];
                // A `<longName>` may contain `<br/>`, which MuseScore lays
                // out as a line break; a plain Edwin run uses a space.
                let clean = |name: &str| name.replace("<br/>", " ");
                Self {
                    index,
                    part: staff.part.index(),
                    long_name: clean(&part.long_name),
                    short_name: clean(&part.short_name),
                    brackets: staff.brackets.clone(),
                    bar_line_span: staff.bar_line_span,
                }
            })
            .collect()
    }

    /// `true` if a bracket or brace starting on this staff covers `span`
    /// staves of the system.
    pub fn bracket_span(&self) -> Option<(i8, usize)> {
        self.brackets
            .iter()
            .find(|bracket| bracket.visible)
            .map(|bracket| (bracket.kind, bracket.span))
    }
}

/// Vertical geometry of one system: the top-line y of each staff relative to
/// the first staff's top line, and the total height to the last bottom line.
///
/// // Port of `SystemLayout::layoutSystem` (`systemlayout.cpp:2488-2582`):
/// `dist = staffHeight + akkoladeDistance * mag` when the next visible staff
/// belongs to the same part, `staffHeight + staffDistance` otherwise. The
/// skyline and spacer corrections are not yet ported (P4); `userDist` is.
pub fn system_geometry(staffs: &[StaffInfo], spec: &PageSpec) -> (Vec<i32>, i32) {
    let mut ys = Vec::with_capacity(staffs.len());
    let mut y = 0;
    for _ in staffs {
        ys.push(y);
        y += STAFF_HEIGHT;
    }
    // Turn the offsets into positions including the intervening distances.
    let mut cursor = 0;
    for (index, position) in ys.iter_mut().enumerate() {
        *position += cursor;
        if index + 1 < staffs.len() {
            cursor += if staffs[index].part == staffs[index + 1].part {
                spec.akkolade_distance
            } else {
                spec.staff_distance
            };
        }
    }
    let height = ys.last().copied().unwrap_or(0) + STAFF_HEIGHT;
    (ys, height)
}
