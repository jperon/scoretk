// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/systemlayout.cpp:2284-2366`
// (`SystemLayout::layoutBrackets`, `addBrackets`, `setBracketsXPosition`),
// `:2714-2805` (`layoutBracketsVertical`, `layoutInstrumentNames`) and
// `tdraw.cpp:1088-1145` (`TDraw::draw(const Bracket*)`) of MuseScore Studio
// 4.7.4 (tag v4.7.4), Copyright (C) MuseScore Limited and others, modified
// (rewritten in Rust) in 2026: brackets and names are emitted directly into
// the page-local display list, with no `System*`/`Bracket*` graph.
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

//! Brackets, braces and instrument names at the left of a system.

use alloc::vec::Vec;

use crate::display::{DisplayItem, DisplayList, ItemKind};
use crate::layout::{place_text_kind, text_em};
use crate::staffs::StaffInfo;
use crate::style::{
    AKKOLADE_BAR_DISTANCE, AKKOLADE_WIDTH, BRACKET_DISTANCE, BRACKET_WIDTH, SP, STAFF_HEIGHT,
    THIN_BARLINE_WIDTH,
};
use crate::{breaking::SystemPlan, layout::PageSpec};
use scoretk_smufl::{glyph_index, units_to_fixed, GLYPHS};

/// Groups of staves whose bar lines span the whole group
/// (`Staff::barLineSpan()`, `systemlayout.cpp:2523-2530`).
pub fn barline_groups(staffs: &[StaffInfo]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < staffs.len() {
        if !staffs[index].bar_line_span || index + 1 >= staffs.len() {
            index += 1;
            continue;
        }
        let part = staffs[index].part;
        let mut end = index;
        // // Port of `nextVisibleSpannedStaff` (`barline.cpp:193-210`): the
        // bar line of a staff with `barLineSpan` reaches the next visible
        // staff of the same part. Only the *first* staff of a grand staff
        // carries `<barLineSpan>1</barLineSpan>` in `.mscx`; requiring the
        // flag on both staves excluded piano/organ groups entirely.
        while end + 1 < staffs.len() && staffs[end + 1].part == part {
            end += 1;
        }
        if end > index {
            groups.push((index, end));
            index = end + 1;
        } else {
            index += 1;
        }
    }
    groups
}

/// Draws the brackets and braces starting on a system's staves.
///
/// // Port of `layoutBracketsVertical` (`systemlayout.cpp:2714-2767`): the
/// bracket spans from the top line of its first visible staff to the bottom
/// line of its last one. The brace is the SMuFL `brace` symbol scaled to
/// that height (`tdraw.cpp:1088-1105`); a normal bracket is a thin vertical
/// rule with the two SMuFL hook glyphs (`tdraw.cpp:1119-1139`).
pub fn place_brackets(out: &mut DisplayList, staffs: &[StaffInfo], system: &SystemPlan, left: i32) {
    let y_at =
        |staff: usize| -> i32 { system.y + system.staff_offsets.get(staff).copied().unwrap_or(0) };
    for (start, staff) in staffs.iter().enumerate() {
        let Some((kind, span)) = staff.bracket_span() else {
            continue;
        };
        let end = (start + span.saturating_sub(1)).min(staffs.len().saturating_sub(1));
        let top = y_at(start) - SP / 2;
        let bottom = y_at(end) + STAFF_HEIGHT + SP / 2;
        let height = bottom - top;
        if kind == 1 {
            let Some(index) = glyph_index("brace") else {
                continue;
            };
            let Some(glyph) = GLYPHS.get(usize::from(index)) else {
                continue;
            };
            let bbox = glyph.bbox;
            let width = units_to_fixed(bbox[2] - bbox[0]).max(1);
            let glyph_height = units_to_fixed(bbox[3] - bbox[1]).max(1);
            // `akkoladeWidth` is the distance between the brace's right edge
            // and the system's staff lines (`tlayout.cpp:1426-1429`).
            let sx = (AKKOLADE_WIDTH.max(1)) * 1024 / width;
            let sy = height * 1024 / glyph_height;
            let x =
                left - AKKOLADE_BAR_DISTANCE - AKKOLADE_WIDTH - units_to_fixed(bbox[0]) * sx / 1024;
            let y = top + units_to_fixed(bbox[3]) * sy / 1024;
            out.push(DisplayItem::scaled(index, x, y, sx, sy));
        } else {
            // A choir square bracket: a line plus two short hooks, inside
            // `bracketWidth` (`tdraw.cpp:1120-1135`).
            let x = left - BRACKET_DISTANCE - BRACKET_WIDTH / 2;
            let bd = SP / 2;
            out.push(DisplayItem::rect(
                ItemKind::Bracket,
                x - BRACKET_WIDTH / 2,
                top - bd,
                BRACKET_WIDTH,
                height + 2 * bd,
            ));
            for (name, y) in [("bracketTop", top - bd), ("bracketBottom", bottom + bd)] {
                if let Some(index) = glyph_index(name) {
                    out.push(DisplayItem::glyph(
                        ItemKind::Bracket,
                        index,
                        x - BRACKET_WIDTH / 2,
                        y,
                    ));
                }
            }
        }
    }
}

/// Instrument names are drawn to the left of the first staff of each part;
/// the first system shows the long name, later systems the short one
/// (`systemlayout.cpp:2748-2805`, `setInstrumentNames` at `:2812-2865`).
pub fn place_instrument_names(
    out: &mut DisplayList,
    spec: &PageSpec,
    staffs: &[StaffInfo],
    system: &SystemPlan,
    left: i32,
    first_system: bool,
) {
    for (index, staff) in staffs.iter().enumerate() {
        if staffs
            .get(index.wrapping_sub(1))
            .is_some_and(|previous| previous.part == staff.part)
        {
            continue;
        }
        let mut end = index;
        while end + 1 < staffs.len() && staffs[end + 1].part == staff.part {
            end += 1;
        }
        let top = system.y + system.staff_offsets.get(index).copied().unwrap_or(0);
        let bottom = system.y + system.staff_offsets.get(end).copied().unwrap_or(0) + STAFF_HEIGHT;
        let size = if first_system { 12.0 } else { 10.0 };
        let em = text_em(spec, size);
        let text = if first_system {
            &staff.long_name
        } else {
            &staff.short_name
        };
        if text.is_empty() {
            continue;
        }
        let bracket = staff.bracket_span().map_or(0, |(kind, _)| {
            if kind == 1 {
                AKKOLADE_WIDTH + AKKOLADE_BAR_DISTANCE
            } else {
                BRACKET_WIDTH + BRACKET_DISTANCE
            }
        });
        let x = left - bracket - spec.instrument_name_offset;
        let y = (top + bottom) / 2 - em * 2 / 5;
        place_text_kind(out, ItemKind::InstrumentName, text, x, y, em, false, 2);
    }
}

/// System-initial bar line spanning a bracketed group of staves
/// (`barLineSpan`): MuseScore draws one stroke from the top line of the group
/// to the bottom line instead of one per staff (`tdraw.cpp:752-805`).
pub fn place_initial_barline(
    out: &mut DisplayList,
    staffs: &[StaffInfo],
    groups: &[(usize, usize)],
    system: &SystemPlan,
    left: i32,
) {
    for &(first, last) in groups {
        let top = system.y + system.staff_offsets.get(first).copied().unwrap_or(0);
        let bottom = system.y + system.staff_offsets.get(last).copied().unwrap_or(0) + STAFF_HEIGHT;
        out.push(DisplayItem::rect(
            ItemKind::BarLine,
            left - THIN_BARLINE_WIDTH / 2,
            top,
            THIN_BARLINE_WIDTH,
            bottom - top,
        ));
    }
    let _ = staffs;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breaking::SystemPlan;
    use crate::style::SP;
    use alloc::vec;
    use scoretk_core::model::StaffBracket;

    fn staff(part: usize, kind: i8, span: usize, bar_line_span: bool) -> StaffInfo {
        StaffInfo {
            index: part,
            part,
            long_name: "Choir".into(),
            short_name: "Ch.".into(),
            brackets: vec![StaffBracket {
                kind,
                span,
                visible: true,
            }],
            bar_line_span,
        }
    }

    fn system(staff_count: usize) -> SystemPlan {
        SystemPlan {
            first: 0,
            count: 1,
            scale: 1024,
            y: 9 * SP,
            page: 0,
            height: 4 * SP,
            staff_offsets: (0..staff_count)
                .map(|index| (index as i32) * 10 * SP)
                .collect(),
        }
    }

    #[test]
    fn barlines_span_the_next_staff_of_a_grand_staff_part() {
        // A piano/organ part writes `<barLineSpan>1</barLineSpan>` only on
        // its first staff (`barline.cpp:193-210` spans to the next staff of
        // the part, whether or not that staff repeats the flag).
        let staffs = vec![
            staff(0, 1, 2, true),
            staff(0, 0, 1, false),
            staff(1, 0, 1, false),
        ];
        assert_eq!(barline_groups(&staffs), vec![(0, 1)]);
    }

    #[test]
    fn brackets_and_braces_are_drawn_with_instrument_names() {
        let staffs = vec![staff(0, 0, 2, false), staff(0, 0, 2, false)];
        let system = system(2);
        let mut out = DisplayList::new(0);
        place_brackets(&mut out, &staffs, &system, 20 * SP);
        place_instrument_names(
            &mut out,
            &PageSpec::default(),
            &staffs,
            &system,
            20 * SP,
            true,
        );
        assert_eq!(out.count(ItemKind::Bracket), 6); // two brackets, each with a line and two hooks
        assert_eq!(out.count(ItemKind::InstrumentName), 5); // "Choir" in Edwin glyphs

        let staffs = vec![staff(1, 1, 2, true), staff(1, 1, 2, true)];
        let mut out = DisplayList::new(0);
        place_brackets(&mut out, &staffs, &system, 20 * SP);
        assert_eq!(out.count(ItemKind::Bracket), 2); // two scaled brace glyphs
    }

    #[test]
    fn bar_line_span_groups_consecutive_staves_of_a_part() {
        let staffs = vec![
            staff(0, 0, 2, true),
            staff(0, 0, 2, true),
            staff(1, 1, 2, false),
        ];
        assert_eq!(barline_groups(&staffs), vec![(0, 1)]);
    }
}
