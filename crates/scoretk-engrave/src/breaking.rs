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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

/// // Port of `SystemLayout::collectSystem` (`:206-225`): append measures
/// until the system's natural width exceeds the printable width, or a line
/// break ends it. At least one measure is kept even if it is too wide.
/// `shouldBeJustified` (`:459-489`) leaves the last system unstretched when
/// it fills less than `lastSystemFillLimit` of the page width and no further
/// measure follows.
fn collect_systems(atoms: &[MeasureAtom], spec: &PageSpec) -> Vec<SystemPlan> {
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
        let mut header_width = 0;
        let mut spring = 0;
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
            if count > 0 && (width + atom.width > printable || atoms[first + count - 1].line_break)
            {
                break;
            }
            header_width += header;
            spring += atom.spring;
            width += atom.width;
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
        let available = printable - header_width - end_bar;
        let scale = if justify && spring > 0 && available > 0 {
            // `stretchSegmentsToWidth`: each spring ends at width
            // `force / springConst = force * stretch`, where `force` is
            // chosen so the stretched widths fill the available space.
            let force = available * 1024 / spring;
            // Natural segment widths are `3.5 sp * stretch`; the new widths
            // are `force * stretch`, so the scale is `force / 3.5 sp`.
            force * 1024 / 3584
        } else {
            1024
        };
        systems.push(SystemPlan {
            first,
            count,
            scale,
            y: 0,
            page: 0,
        });
        first += count;
    }
    systems
}

/// // Port of `PageLayout::collectPage` (`pagelayout.cpp:140-265`): systems
/// are placed from the top margin with `minDistance` between them
/// (`systemlayout.cpp:2900-2991`: 8.5 sp for one staff with no spacers) and
/// a page break once the next system would cross the bottom margin.
fn assign_pages(systems: &mut [SystemPlan], spec: &PageSpec) -> usize {
    let mut page = 0;
    let mut y = spec.margin_top;
    let bottom = spec.page_height - spec.margin_top;
    for system in systems.iter_mut() {
        let distance = if system.y == 0 && system.first == 0 {
            0
        } else {
            spec.min_system_distance
        };
        if y + distance + STAFF_HEIGHT > bottom && system.first > 0 {
            page += 1;
            y = spec.margin_top;
        }
        y += distance;
        system.y = y;
        system.page = page;
        y += STAFF_HEIGHT;
    }
    page + 1
}

/// Computes the systems and pages, together with the atoms augmented with
/// the clef/key headers generated at every system start.
///
/// // Port of `measurelayout.cpp:2530-2590, 2597-2660`: `genClef`/`genKeysig`
/// add header segments to the first measure of a system before spacing, so
/// adding them changes the natural width and can move a break. Iterate until
/// the break pattern is stable (one pass is enough for ordinary scores).
pub fn compute_with_atoms(
    atoms: &[MeasureAtom],
    spec: &PageSpec,
) -> (Vec<SystemPlan>, Vec<MeasureAtom>) {
    let mut augmented = atoms.to_vec();
    let mut systems = collect_systems(&augmented, spec);
    for _ in 0..8 {
        let mut next = atoms.to_vec();
        for system in &systems {
            if system.first > 0 && system.first < atoms.len() {
                next[system.first] = atoms[system.first].with_system_header();
            }
        }
        if next == augmented {
            break;
        }
        augmented = next;
        systems = collect_systems(&augmented, spec);
    }
    assign_pages(&mut systems, spec);
    (systems, augmented)
}

/// Computes all systems and pages from the atoms.
pub fn compute(atoms: &[MeasureAtom], spec: &PageSpec) -> Vec<SystemPlan> {
    compute_with_atoms(atoms, spec).0
}

/// Number of pages of a plan.
pub fn page_count(systems: &[SystemPlan]) -> usize {
    systems.last().map_or(1, |system| system.page + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atoms::{Atom, AtomKind, BarCode, ChordAtom, MeasureAtom, NoteAtom};
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
                },
            ],
            start_bar: BarCode(1),
            end_bar: BarCode(1),
            line_break: false,
            clef_name: "gClef",
            key_fifths: 0,
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
    fn pages_follow_system_heights() {
        let spec = PageSpec::default();
        let atoms: Vec<_> = (0..60).map(|i| quarter(1280 + i * 4864)).collect();
        let systems = compute(&atoms, &spec);
        assert!(page_count(&systems) > 1);
        assert!(systems.iter().any(|s| s.page == 1));
    }
}
