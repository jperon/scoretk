// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/systemlayout.cpp:95-225, 459-489`
// (`SystemLayout::collectSystem`, `shouldBeJustified`) and
// `pagelayout.cpp:119-265` (`PageLayout::collectPage`) of MuseScore Studio
// 4.7.4 (tag v4.7.4), Copyright (C) MuseScore Limited and others, modified
// (rewritten in Rust) in 2026. The `Measure`/`System` DOM is replaced by
// compact atoms: no drawing object is built while breaking lines or pages.
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

//! Line and page breaking on atoms only (plan §4, stage 2).

use alloc::vec::Vec;

use crate::layout::PageSpec;
use crate::style::{BARLINE_SEPARATION, STAFF_HEIGHT, THICK_BARLINE_WIDTH};
use crate::MeasureAtom;

/// One system, described entirely by atom indices and one scale factor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemPlan {
    /// Index of the first measure atom of the system.
    pub first: usize,
    /// Number of measure atoms in the system.
    pub count: usize,
    /// Stretch applied to chord/rest segments; 1024 means natural width.
    pub scale: i32,
    /// Distance of the system's top staff line from the page top.
    pub y: i32,
    /// Page that holds this system (0-based).
    pub page: usize,
    /// Height of all staves of the system, top line to bottom line.
    pub height: i32,
    /// Top-line y of each staff, relative to `y` (the first staff's top line).
    pub staff_offsets: Vec<i32>,
}

/// // Port of `SystemLayout::collectSystem` (`:206-225`): append measures
/// until the system's natural width exceeds the printable width, or a line
/// break ends it. At least one measure is kept even if it is too wide.
/// `shouldBeJustified` (`:459-489`) leaves the last system unstretched when
/// it fills less than `lastSystemFillLimit` of the page width and no further
/// measure follows.
fn collect_systems(atoms: &[MeasureAtom], spec: &PageSpec, multistaff: bool) -> Vec<SystemPlan> {
    let mut systems = Vec::new();
    let mut first = 0;
    while first < atoms.len() {
        let printable = spec.printable_width
            - if first == 0 {
                spec.first_system_indent
            } else {
                0
            };
        let mut count = 0;
        let mut width = 0;
        let mut fixed_extra = 0;
        let mut stretch_sum = 0;
        // Attached text shapes have a fixed minimum width: justification
        // fills only the space left after them (`horizontalspacing.cpp:
        // 1087-1111`), it does not stretch them again.
        let mut min_extra = 0;
        while first + count < atoms.len() {
            let atom = &atoms[first + count];
            // // Port of `horizontalspacing.cpp:1087-1111`: justification
            // stretches only the chord/rest springs. Everything before the
            // first chord (clef/key/time widths and barNoteDistance) is a
            // fixed header; using the first *event* here counted only the
            // clef's x and left 6 sp out of the spring budget.
            let header = atom
                .events
                .iter()
                .find(|event| {
                    // Grace notes share the segment of the following chord
                    // and sit to its left; they are not a header segment.
                    !event.grace
                        && matches!(
                            event.kind,
                            crate::AtomKind::Chord | crate::AtomKind::Rest { .. }
                        )
                })
                .map_or(atom.width, |event| event.x);
            // With several staves, MuseScore packs measures using their
            // *minimum* widths (`horizontalspacing.cpp:1216-1276`) rather
            // than the natural 3.5 sp springs used by the single-staff P1
            // oracle: a dense organ/choir measure must be allowed to shrink.
            // The 3/5 estimate is used only for the break decision; the
            // segment x positions are still justified from the real springs.
            let break_width = if multistaff {
                header + (atom.width - header) * 3 / 5
            } else {
                atom.width
            };
            if count > 0 && (width + break_width > printable || atoms[first + count - 1].line_break)
            {
                break;
            }
            let (fixed, stretch) = crate::layout::stretch_budget(atom);
            fixed_extra += fixed;
            stretch_sum += stretch;
            min_extra += atom.min_extra;
            width += break_width;
            count += 1;
            if atom.line_break {
                break;
            }
        }
        // // Port of `SystemLayout::shouldBeJustified`
        // (`systemlayout.cpp:459-489`): a system whose natural width is
        // below `lastSystemFillLimit` of the target width is justified
        // **only when a measure follows it**. A short final system — e.g.
        // `copypaste_parts`'s last system with one whole-measure rest —
        // stays at its natural width. Step 7 justified it unconditionally,
        // a regression from eng2b; `testNoteheads2`'s final system fills
        // more than 0.3 and is still justified.
        let is_last = first + count >= atoms.len();
        let fill = width * 1024 / spec.printable_width.max(1);
        let justify = !(is_last && fill < spec.last_system_fill_limit);
        // The last fixed segment of the final system is the generated end
        // barline (`endBarWidth` + `endBarDistance`), not a spring
        // (`horizontalspacing.cpp:1135-1152`).
        let end_bar = if atoms[first + count - 1].end_bar.0 == 3 {
            THICK_BARLINE_WIDTH + BARLINE_SEPARATION
        } else {
            0
        };
        let available = (printable - fixed_extra - end_bar - min_extra).max(0);
        let scale = if justify && stretch_sum > 0 && available > 0 {
            // `stretchSegmentsToWidth` (`horizontalspacing.cpp:1087-1111`):
            // the force fills the space left by fixed headers and minimum
            // shape widths. Solving it against the measured natural segment
            // widths, rather than `3.5 sp * spring`, keeps a short or dotted
            // note from stretching the system past the right margin.
            available * 1024 / stretch_sum
        } else {
            1024
        };
        systems.push(SystemPlan {
            first,
            count,
            scale,
            y: 0,
            page: 0,
            height: 0,
            staff_offsets: alloc::vec![0],
        });
        first += count;
    }
    systems
}

/// // Port of `PageLayout::collectPage` (`pagelayout.cpp:140-265`): systems
/// are placed from the top margin with `minDistance` between them
/// (`systemlayout.cpp:2900-2991`: 8.5 sp for one staff with no spacers) and
/// a page break once the next system would cross the bottom margin.
fn assign_pages(systems: &mut [SystemPlan], spec: &PageSpec, heights: &[i32]) -> usize {
    let mut page = 0;
    // The first-page title frame (VBox + `frameSystemDistance`) precedes the
    // first system (`boxlayout.cpp`, `systemlayout.cpp:2900-2991`).
    let mut y = spec.margin_top + spec.title_frame_height;
    let bottom = spec.page_height - spec.margin_top;
    for (index, system) in systems.iter_mut().enumerate() {
        let distance = if system.y == 0 && system.first == 0 {
            0
        } else {
            spec.min_system_distance
        };
        let height = heights.get(index).copied().unwrap_or(STAFF_HEIGHT);
        system.height = height;
        if y + distance + height > bottom && system.first > 0 {
            page += 1;
            // // Port of `pagelayout.cpp:179`: the first system of a page
            // starts below `staffUpperBorder`.
            y = spec.margin_top + spec.staff_upper_border;
        }
        y += distance;
        system.y = y;
        system.page = page;
        y += height;
    }
    page + 1
}

/// // Port of `PageLayout::collectPage`'s page-break loop
/// (`pagelayout.cpp:140-265`): decide whether the next system still fits
/// using the *measured* distance from the skyline pass and the system's
/// actual height, not the nominal style distance. Page breaks stay honest:
/// a system moved to a new page starts below `staffUpperBorder`.
pub fn repaginate(systems: &mut [SystemPlan], spec: &PageSpec) {
    if systems.is_empty() {
        return;
    }

    let gaps: alloc::vec::Vec<i32> = systems
        .windows(2)
        .map(|pair| (pair[1].y - pair[0].y - pair[0].height).max(spec.min_system_distance))
        .collect();
    let bottom = spec.page_height - spec.margin_top - spec.staff_lower_border;
    let mut page = 0;
    let mut y = spec.margin_top + spec.title_frame_height;
    for (index, system) in systems.iter_mut().enumerate() {
        if index > 0 {
            let gap = gaps[index - 1];
            if y + gap + system.height > bottom {
                page += 1;
                y = spec.margin_top + spec.staff_upper_border;
            }
            y += gap;
        }
        system.y = y;
        system.page = page;
        y += system.height;
    }
}

/// Computes the systems and pages, together with the atoms augmented with
/// the clef/key headers generated at every system start.
///
/// // Port of `measurelayout.cpp:2530-2590, 2597-2660`: `genClef`/`genKeysig`
/// add header segments to the first measure of a system before spacing, so
/// adding them changes the natural width and can move a break. Iterate until
/// the break pattern is stable (one pass is enough for ordinary scores).
/// One measure as seen by the breaking pass: the largest width and spring
/// over all staves at each segment, as `HorizontalSpacing` visits a segment
/// once for the whole system (`horizontalspacing.cpp:47-89, 262-327`).
fn merged_measure(all: &[Vec<MeasureAtom>], measure: usize) -> MeasureAtom {
    let mut merged = all[0][measure].clone();
    for atoms in all.iter().skip(1) {
        let atom = &atoms[measure];
        if atom.width > merged.width {
            merged.events = atom.events.clone();
        }
        merged.width = merged.width.max(atom.width);
        merged.spring = merged.spring.max(atom.spring);
        merged.start_bar.0 = merged.start_bar.0.max(atom.start_bar.0);
        merged.end_bar.0 = merged.end_bar.0.max(atom.end_bar.0);
        merged.line_break |= atom.line_break;
        merged.header_width = merged.header_width.max(atom.header_width);
        merged.min_extra = merged.min_extra.max(atom.min_extra);
    }
    merged
}

fn merged_score(all: &[Vec<MeasureAtom>]) -> Vec<MeasureAtom> {
    (0..all[0].len())
        .map(|measure| merged_measure(all, measure))
        .collect()
}

/// Gives every staff of a system the same header width, the maximum of the
/// per-staff generated clef/key widths (`measurelayout.cpp:2530-2660`).
fn align_headers(all: &mut [Vec<MeasureAtom>], first: usize) {
    let common = all
        .iter()
        .map(|atoms| atoms[first].header_width)
        .max()
        .unwrap_or(0);
    for atoms in all.iter_mut() {
        let atom = &mut atoms[first];
        let delta = common - atom.header_width;
        if delta == 0 {
            continue;
        }
        atom.header_width = common;
        atom.width += delta;
        for base in &mut atom.segment_bases {
            *base += delta;
        }
        for event in &mut atom.events {
            if event.segment != u16::MAX {
                event.x += delta;
            }
        }
    }
}

pub fn compute_with_atoms(
    all: &[Vec<MeasureAtom>],
    spec: &PageSpec,
    staffs: &[crate::staffs::StaffInfo],
    system_heights: &[i32],
) -> (Vec<SystemPlan>, Vec<Vec<MeasureAtom>>) {
    let mut augmented = all.to_vec();
    let mut systems = collect_systems(&merged_score(&augmented), spec, all.len() > 1);
    for _ in 0..8 {
        let mut next = all.to_vec();
        for system in &systems {
            if system.first > 0 && system.first < all[0].len() {
                // Each staff keeps its own clef/key header, built from its
                // own default clef and key (`measurelayout.cpp:2530-2660`).
                for (staff, atoms) in next.iter_mut().enumerate() {
                    if let Some(source) = all.get(staff) {
                        atoms[system.first] = source[system.first].with_system_header();
                    }
                }
                align_headers(&mut next, system.first);
            }
        }
        if next == augmented {
            break;
        }
        augmented = next;
        systems = collect_systems(&merged_score(&augmented), spec, all.len() > 1);
    }
    for system in &mut systems {
        system.staff_offsets = alloc::vec![0; all.len()];
    }
    // Before any skyline is known, every staff of a system uses the style
    // distances (`systemlayout.cpp:2485-2532`). The skyline pass in
    // `Engraver::new` can only widen these offsets, never narrow them.
    let (mut offsets, _) = crate::staffs::system_geometry(staffs, spec);
    if offsets.is_empty() {
        offsets.push(0);
    }
    for system in &mut systems {
        system.staff_offsets = offsets.clone();
    }
    assign_pages(&mut systems, spec, system_heights);
    (systems, augmented)
}

/// Computes all systems and pages from one staff's atoms (used by the unit
/// tests and by callers that already collapsed the staves).
pub fn compute(atoms: &[MeasureAtom], spec: &PageSpec) -> Vec<SystemPlan> {
    let heights = alloc::vec![STAFF_HEIGHT; atoms.len()];
    let mut systems = compute_with_atoms(&[atoms.to_vec()], spec, &[], &heights).0;
    crate::vertical::spread_all(&mut systems, &[], spec);
    systems
}

/// Number of pages of a plan.
pub fn page_count(systems: &[SystemPlan]) -> usize {
    systems.last().map_or(1, |system| system.page + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atoms::{Atom, AtomKind, BarCode, ChordAtom, MeasureAtom, NoteAtom};
    use crate::style::SP;
    use alloc::vec;

    fn quarter(at: i32) -> MeasureAtom {
        MeasureAtom {
            number: 0,
            width: at + 3584 + 1280,
            spring: 1024,
            events: vec![
                Atom::simple(
                    1280,
                    AtomKind::Rest {
                        duration: 4,
                        dots: 0,
                        measure: false,
                    },
                ),
                Atom {
                    x: at,
                    kind: AtomKind::Chord,
                    chord: Some(ChordAtom {
                        duration: 4,
                        dots: 0,
                        grace: false,
                        stem_up: true,
                        stem_len: 3072,
                        notes: vec![NoteAtom {
                            line: 4,
                            head: "noteheadBlack",
                            accidental: None,
                        }],
                    }),
                    voice: 0,
                    measure: 0,
                    beams: 0,
                    beam_mode: scoretk_core::model::BeamMode::Auto,
                    beam_start: false,
                    beam_dir: None,
                    grace: false,
                    ..Default::default()
                },
            ],
            start_bar: BarCode(1),
            end_bar: BarCode(1),
            line_break: false,
            clef_name: "gClef",
            key_fifths: 0,
            mm_rest_count: 1,
            ..Default::default()
        }
    }

    #[test]
    fn measures_are_grouped_by_printable_width() {
        // A narrow printable width forces a break after every measure.
        let spec = PageSpec {
            printable_width: 8 * 1024,
            ..PageSpec::default()
        };
        let atoms: Vec<_> = (0..5).map(|i| quarter(1280 + i * 4864)).collect();
        let systems = compute(&atoms, &spec);
        assert!(systems.len() >= 2);
        assert_eq!(systems[0].first, 0);
        assert_eq!(systems.iter().map(|s| s.count).sum::<usize>(), 5);
        assert!(systems.iter().all(|s| s.scale > 0));
    }

    #[test]
    fn a_short_last_system_is_left_unstretched() {
        // `shouldBeJustified` (systemlayout.cpp:459-489): a system below
        // `lastSystemFillLimit` is justified only when a measure follows.
        // `copypaste_parts`'s final system holds one whole-measure rest and
        // MuseScore leaves it short.
        let spec = PageSpec::default();
        let atoms = vec![quarter(1280)];
        let systems = compute(&atoms, &spec);
        assert_eq!(systems.len(), 1);
        assert_eq!(systems[0].scale, 1024);
    }

    #[test]
    fn a_short_non_final_system_is_justified() {
        let spec = PageSpec::default();
        // One measure per system (each carries a line break), each below
        // `lastSystemFillLimit`. Every system but the last is followed by a
        // measure and is justified; the last one is left at natural width.
        let atoms: Vec<_> = (0..4)
            .map(|_| {
                let mut atom = quarter(1280);
                atom.width = 20 * 1024;
                atom.line_break = true;
                atom
            })
            .collect();
        let systems = compute(&atoms, &spec);
        assert_eq!(systems.len(), 4);
        assert!(systems[..3].iter().all(|system| system.scale > 1024));
        assert_eq!(systems[3].scale, 1024);
    }

    #[test]
    fn page_breaks_use_the_measured_system_heights() {
        // `pagelayout.cpp:248-265`: a tall system can force a page break
        // even when the nominal `minSystemDistance` would still fit.
        let spec = PageSpec::default();
        let system = |y: i32, height: i32| SystemPlan {
            first: 0,
            count: 1,
            scale: 1024,
            y,
            page: 0,
            height,
            staff_offsets: vec![0],
        };
        let mut systems = vec![system(spec.margin_top, 20 * SP), system(200 * SP, 4 * SP)];
        repaginate(&mut systems, &spec);
        assert_eq!(systems[0].page, 0);
        assert_eq!(systems[1].page, 1);
        assert!(systems[1].y >= spec.margin_top + spec.staff_upper_border);
    }

    #[test]
    fn pages_follow_system_heights() {
        let spec = PageSpec::default();
        let atoms: Vec<_> = (0..60).map(|i| quarter(1280 + i * 4864)).collect();
        let systems = compute(&atoms, &spec);
        assert!(page_count(&systems) > 1);
        assert!(systems.iter().any(|s| s.page == 1));
    }
}
