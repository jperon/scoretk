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
use crate::style::STAFF_HEIGHT;
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
        let mut count = 0;
        let mut width = 0;
        let mut header_width = 0;
        let mut spring = 0;
        while first + count < atoms.len() {
            let atom = &atoms[first + count];
            let header = atom.events.first().map_or(atom.width, |event| event.x);
            if count > 0
                && (width + atom.width > spec.printable_width
                    || atoms[first + count - 1].line_break)
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
        let is_last = first + count == atoms.len();
        let fill = width * 1024 / spec.printable_width.max(1);
        let justify = !is_last || fill >= spec.last_system_fill_limit;
        let available = spec.printable_width - header_width;
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

/// Computes all systems and pages from the atoms.
pub fn compute(atoms: &[MeasureAtom], spec: &PageSpec) -> Vec<SystemPlan> {
    let mut systems = collect_systems(atoms, spec);
    assign_pages(&mut systems, spec);
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
                        stem_up: true,
                        stem_len: 3072,
                        notes: vec![NoteAtom {
                            line: 4,
                            head: "noteheadBlack",
                            accidental: None,
                        }],
                    }),
                },
            ],
            start_bar: BarCode(1),
            end_bar: BarCode(1),
            line_break: false,
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
        assert!(systems.iter().all(|s| s.scale >= 1024));
    }

    #[test]
    fn a_short_last_system_is_not_justified() {
        let spec = PageSpec::default();
        let atoms = vec![quarter(1280)];
        let systems = compute(&atoms, &spec);
        assert_eq!(systems.len(), 1);
        assert_eq!(systems[0].scale, 1024);
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
