// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/pagelayout.cpp:614-820`
// (`PageLayout::distributeStaves`),
// `src/engraving/rendering/score/verticalgapdata.cpp:33-232`
// (`VerticalGapData`, `VerticalGapDataList`),
// `src/engraving/rendering/score/systemlayout.cpp:2485-2581, 2900-2989`
// (skyline distances between staves and systems) and
// `src/engraving/infrastructure/skyline.cpp:198-216` (`Skyline::minDistance`)
// of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright (C) MuseScore Limited
// and others, modified (rewritten in Rust) in 2026: gaps are plain
// fixed-point numbers, with no `System*`, `SysStaff*` or item graph.
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

//! Vertical justification of one page (`enableVerticalSpread`, the default
//! in 4.7.4). MuseScore's `layoutPage` leaves the distances untouched when
//! spreading is enabled and calls `distributeStaves` instead
//! (`pagelayout.cpp:549-558`): the gaps above every staff are equalised
//! according to their stretch factor, then the remaining page space is
//! distributed up to `maxPageFillSpread`.

use alloc::vec::Vec;

use crate::breaking::SystemPlan;
use crate::layout::PageSpec;
use crate::skyline::Skyline;
use crate::staffs::StaffInfo;
use crate::style::STAFF_HEIGHT;

/// One gap above a staff. Values are 1/1024 staff space; factors are also
/// fixed point (1024 = 1.0), as are the maximum spacings.
#[derive(Clone, Copy, Debug)]
struct Gap {
    /// Distance from the bottom line of the preceding staff to the top line
    /// of this staff, before spreading.
    normalised: i32,
    /// `spreadSystem`, `spreadSquareBracket` or `spreadCurlyBracket`.
    factor: i32,
    /// Maximum normalised spacing (`maxSystemSpread`, `maxStaffSpread`,
    /// `maxAkkoladeDistance`), divided by the factor as in
    /// `verticalgapdata.cpp:80-84, 114-128`.
    maximum: i32,
    /// Space already added, in normalised units.
    added: i32,
    /// Last step, to be able to undo it (`verticalgapdata.cpp:180-186`).
    last_step: i32,
    /// Space added by the final fill pass, in actual units.
    fill: i32,
    fixed: bool,
}

impl Gap {
    fn spacing(&self) -> i32 {
        self.normalised + self.added
    }

    /// `VerticalGapData::isFixedHeight` (`verticalgapdata.cpp:177-180`): a
    /// gap at or above its maximum cannot grow. A skyline-forced gap may be
    /// larger than the nominal maximum; it must not be shrunk back to it.
    fn is_fixed(&self) -> bool {
        self.fixed || self.normalised >= self.maximum
    }

    fn add(&mut self, step: i32) -> i32 {
        if self.is_fixed() {
            return 0;
        }
        let mut step = step;
        if self.normalised + self.added + step >= self.maximum {
            step = self.maximum - self.normalised - self.added;
        }
        self.added += step;
        self.last_step = step;
        step
    }

    /// Full distance in staff spaces: the initial spacing is unchanged,
    /// only the added space is scaled by the stretch factor
    /// (`verticalgapdata.cpp:143-147`).
    fn distance(&self) -> i32 {
        self.normalised + self.added * self.factor / 1024
    }

    fn sum_factors(gaps: &[Gap]) -> i32 {
        gaps.iter()
            .filter(|gap| !gap.is_fixed())
            .map(|gap| gap.factor)
            .sum()
    }

    /// `VerticalGapDataList::smallest(limit)` (`verticalgapdata.cpp:227-241`).
    fn smallest(gaps: &[Gap], limit: i32) -> i32 {
        let mut best: Option<i32> = None;
        for gap in gaps {
            if gap.is_fixed() || (limit + 1023) / 1024 == (gap.spacing() + 1023) / 1024 {
                continue;
            }
            best = Some(best.map_or(gap.spacing(), |value| value.min(gap.spacing())));
        }
        best.unwrap_or(0)
    }
}

/// Gaps above the staves of one page, in visual order.
fn page_gaps(
    systems: &[SystemPlan],
    staffs: &[StaffInfo],
    spec: &PageSpec,
    page: usize,
) -> Vec<Gap> {
    let mut gaps = Vec::new();
    let on_page: Vec<&SystemPlan> = systems
        .iter()
        .filter(|system| system.page == page)
        .collect();
    for (index, system) in on_page.iter().enumerate() {
        for staff_index in 0..system.staff_offsets.len() {
            if index == 0 && staff_index == 0 {
                // `VerticalGapData` for the first staff on a page has
                // `m_fixedHeight` set (`pagelayout.cpp:629,179`,
                // `verticalgapdata.cpp:35-39`).
                let fixed = if page == 0 && spec.title_frame_height > 0 {
                    spec.frame_system_distance
                } else {
                    spec.staff_upper_border
                };
                gaps.push(Gap {
                    normalised: fixed,
                    factor: 1024,
                    maximum: fixed,
                    added: 0,
                    last_step: 0,
                    fill: 0,
                    fixed: true,
                });
                continue;
            }
            let bracket = staffs.get(staff_index).and_then(StaffInfo::bracket_span);
            // The gap is the *measured* one, after the skyline pass. Spreading
            // only ever adds space: a skyline-forced distance is never shrunk
            // back to the nominal style distance (`verticalgapdata.cpp:35-54`).
            let (normalised, factor, maximum) = if staff_index == 0 {
                let previous = on_page[index - 1];
                (
                    system.y - previous.y - previous.height,
                    spec.spread_system,
                    spec.max_system_spread * 1024 / spec.spread_system.max(1),
                )
            } else {
                let (factor, max_spacing) = match bracket {
                    Some((1, _)) => (
                        spec.spread_curly_bracket,
                        spec.max_akkolade_distance.min(spec.max_staff_spread),
                    ),
                    Some((0 | 3, _)) => (spec.spread_square_bracket, spec.max_staff_spread),
                    _ => (1024, spec.max_staff_spread),
                };
                (
                    system.staff_offsets[staff_index]
                        - system.staff_offsets[staff_index - 1]
                        - STAFF_HEIGHT,
                    factor,
                    max_spacing * 1024 / factor.max(1),
                )
            };
            gaps.push(Gap {
                normalised,
                factor,
                maximum,
                added: 0,
                last_step: 0,
                fill: 0,
                fixed: false,
            });
        }
    }
    gaps
}

/// // Port of `PageLayout::distributeStaves` (`pagelayout.cpp:697-790`):
/// equalise the smallest gaps first, then spend the remaining page space in
/// bounded fill steps.
fn spread(gaps: &mut [Gap], mut remaining: i32, spec: &PageSpec) {
    if remaining <= 0 || !spec.enable_vertical_spread {
        return;
    }
    for _ in 0..20 {
        let smallest = Gap::smallest(gaps, 0);
        let mut next = Gap::smallest(gaps, smallest);
        // `pagelayout.cpp:718-720`: a zero smallest or next-smallest ends
        // the equalisation pass (all gaps already have the same spacing).
        if smallest == 0 || next == 0 {
            break;
        }
        let sum = Gap::sum_factors(gaps);
        if sum == 0 {
            break;
        }
        if (next - smallest) * sum / 1024 > remaining {
            next = smallest + remaining * 1024 / sum;
        }
        let mut added = 0;
        let mut modified: Vec<usize> = Vec::new();
        for (index, gap) in gaps.iter_mut().enumerate() {
            if gap.is_fixed() || gap.spacing() != smallest {
                continue;
            }
            let step = gap.add(next - gap.spacing());
            if step != 0 {
                added += step * gap.factor / 1024;
                modified.push(index);
            }
            if remaining - added <= 0 {
                break;
            }
        }
        if remaining - added <= 0 {
            for index in modified {
                let gap = &mut gaps[index];
                gap.added -= gap.last_step;
                gap.last_step = 0;
            }
            break;
        }
        remaining -= added;
    }

    // `pagelayout.cpp:754-787`: leftover space is distributed to all gaps,
    // capped by `maxPageFillSpread` per gap.
    remaining = remaining.min(spec.max_page_fill_spread * gaps.len() as i32);
    for _ in 0..20 {
        if remaining <= 0 {
            break;
        }
        let sum = Gap::sum_factors(gaps);
        if sum == 0 {
            break;
        }
        let step = remaining * 1024 / sum;
        let mut added = 0;
        for gap in gaps.iter_mut() {
            if gap.is_fixed() {
                continue;
            }
            // `factor` is fixed point (1024 = 1.0): convert the fill already
            // spent in *actual* units back to staff spaces before comparing
            // it with `maxPageFillSpread`. The old division by 1024-scaled
            // factor let a single gap absorb the whole page's free space.
            let fill_spent = gap.fill * 1024 / gap.factor.max(1);
            let fill_step = if (step + fill_spent) > spec.max_page_fill_spread {
                (spec.max_page_fill_spread - fill_spent).max(0)
            } else {
                step
            };
            let step = gap.add(fill_step);
            gap.fill += step * gap.factor / 1024;
            added += step * gap.factor / 1024;
        }
        remaining -= added;
    }
}

/// // Port of the vertical part of `SystemLayout::layoutSystem`
/// (`systemlayout.cpp:2485-2581`) and `SystemLayout::minDistance`
/// (`:2900-2989`): the style distance is a floor, and any item protruding
/// past the staff lines adds `minVerticalDistance` to it.
fn system_gap(upper: &Skyline, lower: &Skyline, spec: &PageSpec) -> i32 {
    // `sld -= sysStaff->bbox().height() - minVerticalDistance`
    // (`systemlayout.cpp:2954-2955`). Both skylines are staff-local, so the
    // upper one's south already includes the staff height.
    let skyline_gap =
        upper.south().max() - STAFF_HEIGHT + spec.min_vertical_distance - lower.north().max();
    // `minDistance` picks `minSystemSpread` when vertical spreading is on
    // (`systemlayout.cpp:2932`).
    let style_gap = if spec.enable_vertical_spread {
        spec.min_system_spread
    } else {
        spec.min_system_distance
    };
    style_gap.max(skyline_gap)
}

/// Applies the skylines measured on a preliminary layout: widens the offsets
/// between adjacent staves and the distances between consecutive systems.
/// Nothing here can reduce a style distance (`systemlayout.cpp:2574, 2989`).
pub fn apply_skylines(
    systems: &mut [SystemPlan],
    skylines: &[Vec<Vec<Skyline>>],
    staffs: &[StaffInfo],
    spec: &PageSpec,
) {
    for (page, page_skylines) in skylines.iter().enumerate() {
        let mut previous: Option<(i32, i32, Skyline)> = None;
        let on_page = systems.iter_mut().filter(|system| system.page == page);
        for (system, staff_skylines) in on_page.zip(page_skylines.iter()) {
            let mut top = 0;
            for staff_index in 1..system.staff_offsets.len().min(staff_skylines.len()) {
                let same_part = staffs
                    .get(staff_index)
                    .zip(staffs.get(staff_index - 1))
                    .is_some_and(|(staff, previous)| staff.part == previous.part);
                let style_distance = if same_part {
                    spec.akkolade_distance
                } else {
                    spec.staff_distance
                };
                // `dist = staffHeight + distance` then
                // `dist = max(dist, d + minVerticalDistance)`: top-line to
                // top-line, hence `STAFF_HEIGHT` is included in the base.
                let base = top + STAFF_HEIGHT + style_distance;
                let d = staff_skylines[staff_index - 1]
                    .south()
                    .min_distance(staff_skylines[staff_index].north(), spec.skyline_clearance)
                    .map(|d| d + spec.min_vertical_distance);
                top = d.map_or(base, |d| base.max(d));
                system.staff_offsets[staff_index] = top;
            }
            system.height = system.staff_offsets.last().copied().unwrap_or(0) + STAFF_HEIGHT;
            if let Some((previous_y, previous_height, previous_skyline)) = &previous {
                if let Some(first_staff) = staff_skylines.first() {
                    system.y = previous_y
                        + previous_height
                        + system_gap(previous_skyline, first_staff, spec);
                }
            }
            if let Some(last_staff) = staff_skylines.last() {
                previous = Some((system.y, system.height, last_staff.clone()));
            }
        }
    }
}

/// // Port of `PageLayout::layoutPage` (`pagelayout.cpp:524-613`) for the
/// case `enableVerticalSpread == false`: normalise the distances between
/// systems with the free page height, but never exceed `maxSystemDistance`.
/// Systems with a VBox or a fixed down distance are left untouched
/// (`pagelayout.cpp:542-545`); neither occurs in this page-local model yet.
fn distribute_without_spread(systems: &mut [SystemPlan], spec: &PageSpec, page: usize) {
    let on_page: alloc::vec::Vec<usize> = systems
        .iter()
        .enumerate()
        .filter(|(_, system)| system.page == page)
        .map(|(index, _)| index)
        .collect();
    if on_page.len() < 2 {
        return;
    }
    let mut distances: alloc::vec::Vec<i32> = on_page
        .windows(2)
        .map(|pair| systems[pair[1]].y - systems[pair[0]].y)
        .collect();
    let heights: alloc::vec::Vec<i32> =
        on_page.iter().map(|&index| systems[index].height).collect();
    let last = *on_page.last().unwrap();
    let bottom = spec.page_height - spec.margin_top - spec.staff_lower_border;
    let mut remaining = bottom
        - (systems[last].y
            + systems[last].staff_offsets.last().copied().unwrap_or(0)
            + STAFF_HEIGHT);
    if remaining <= 0 {
        return;
    }
    // `pagelayout.cpp:564`: shortest bottom-to-top gap first.
    let mut order: alloc::vec::Vec<usize> = (0..distances.len()).collect();
    order.sort_by_key(|&index| distances[index] - heights[index]);
    for position in 1..order.len() {
        let (shortest, taller) = (order[position - 1], order[position]);
        let normal = distances[shortest] - heights[shortest];
        let next = distances[taller] - heights[taller];
        if next <= normal {
            continue;
        }
        let mut fill = next - normal;
        let wanted = fill * position as i32;
        if wanted > remaining {
            fill = remaining / position as i32;
        }
        let total = (fill * position as i32).min(remaining);
        for &index in order.iter().take(position) {
            let candidate = distances[index] + fill;
            distances[index] = if candidate - heights[index] > spec.max_system_distance {
                (spec.max_system_distance + heights[index]).max(distances[index])
            } else {
                candidate
            };
        }
        remaining -= total;
        if remaining <= 0 {
            break;
        }
    }
    if remaining > 0 {
        let step = remaining / distances.len() as i32;
        for index in 0..distances.len() {
            let candidate = distances[index] + step;
            distances[index] = if candidate - heights[index] > spec.max_system_distance {
                (spec.max_system_distance + heights[index]).max(distances[index])
            } else {
                candidate
            };
        }
    }
    // `pagelayout.cpp:604-611`: every y is the accumulated distance.
    let mut y = systems[on_page[0]].y;
    for (index, &system_index) in on_page.iter().enumerate() {
        systems[system_index].y = y;
        if index < distances.len() {
            y += distances[index];
        }
    }
}

/// Rewrites the system positions and per-staff offsets of one page, filling
/// the page instead of packing every system against the top margin.
pub fn spread_page(systems: &mut [SystemPlan], staffs: &[StaffInfo], spec: &PageSpec, page: usize) {
    let mut gaps = page_gaps(systems, staffs, spec, page);
    let last_bottom = systems
        .iter()
        .rev()
        .find(|system| system.page == page)
        .map(|system| system.y + system.staff_offsets.last().copied().unwrap_or(0) + STAFF_HEIGHT);
    let Some(last_bottom) = last_bottom else {
        return;
    };
    let bottom = spec.page_height - spec.margin_top - spec.staff_lower_border;
    spread(&mut gaps, bottom - last_bottom, spec);

    let mut gap_index = 0;
    let mut y = 0;
    let mut first = true;
    for system in systems.iter_mut().filter(|system| system.page == page) {
        if first {
            y = system.y;
            first = false;
        }
        // Gap above the first staff of this system (or the fixed page frame
        // above the very first one, which `assign_pages` already applied).
        let top_gap = gaps[gap_index];
        gap_index += 1;
        if !top_gap.fixed {
            y += top_gap.distance();
        }
        system.y = y;
        system.staff_offsets[0] = 0;
        let mut offset = 0;
        for staff_index in 1..system.staff_offsets.len() {
            let gap = gaps[gap_index];
            gap_index += 1;
            // A gap is measured from the previous staff's *bottom* line to
            // this staff's top line: the offsets are top-line to top-line,
            // so the staff height must be added as well. The old code used
            // the gap alone, putting the staves 4 sp too close.
            offset += STAFF_HEIGHT + gap.distance();
            system.staff_offsets[staff_index] = offset;
        }
        system.height = offset + STAFF_HEIGHT;
        y += offset + STAFF_HEIGHT;
    }
}

/// Page filling, dispatching on `enableVerticalSpread` as
/// `PageLayout::layoutPage` does (`pagelayout.cpp:548-558`).
pub fn distribute_page(
    systems: &mut [SystemPlan],
    staffs: &[StaffInfo],
    spec: &PageSpec,
    page: usize,
) {
    if spec.enable_vertical_spread {
        spread_page(systems, staffs, spec, page);
    } else {
        distribute_without_spread(systems, spec, page);
    }
}

/// Applies page filling to every page of a breaking plan.
pub fn distribute_all(systems: &mut [SystemPlan], staffs: &[StaffInfo], spec: &PageSpec) {
    let pages = systems.last().map_or(1, |system| system.page + 1);
    for page in 0..pages {
        distribute_page(systems, staffs, spec, page);
    }
}

/// Kept for callers using the spreading entry point directly; with spread
/// disabled it now follows the `maxSystemDistance` path as well.
pub fn spread_all(systems: &mut [SystemPlan], staffs: &[StaffInfo], spec: &PageSpec) {
    distribute_all(systems, staffs, spec);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::SP;
    use alloc::string::String;
    use alloc::vec;

    fn system(first: usize) -> SystemPlan {
        SystemPlan {
            first,
            count: 1,
            scale: 1024,
            y: 8 * SP,
            page: 0,
            height: STAFF_HEIGHT,
            staff_offsets: vec![0],
        }
    }

    fn plan(y: i32, offsets: Vec<i32>) -> SystemPlan {
        SystemPlan {
            first: 0,
            count: 1,
            scale: 1024,
            y,
            page: 0,
            height: offsets.last().copied().unwrap_or(0) + STAFF_HEIGHT,
            staff_offsets: offsets,
        }
    }

    fn staff_skyline() -> Skyline {
        let mut skyline = Skyline::new();
        skyline.add_staff_line(0, 0, 10 * SP, STAFF_HEIGHT);
        skyline
    }

    #[test]
    fn a_protruding_stem_widens_the_distance_to_the_next_staff() {
        // `systemlayout.cpp:2564-2575`: `max(styleDistance, d +
        // minVerticalDistance)`, where `d` includes each staff's own height.
        let spec = PageSpec {
            staff_distance: 6 * SP,
            min_vertical_distance: SP / 2,
            skyline_clearance: SP / 4,
            ..PageSpec::default()
        };
        let mut systems = vec![plan(0, vec![0, 10 * SP])];
        let mut upper = staff_skyline();
        upper.add(SP, STAFF_HEIGHT, SP + 100, 12 * SP, STAFF_HEIGHT, 0);
        let skylines = vec![vec![upper, staff_skyline()]];
        apply_skylines(&mut systems, &[skylines], &[], &spec);
        assert_eq!(systems[0].staff_offsets[1], 25 * SP / 2);
        assert!(systems[0].height > 12 * SP);
    }

    #[test]
    fn a_protruding_item_widens_the_distance_to_the_next_system() {
        let spec = PageSpec {
            min_system_distance: 8 * SP,
            min_vertical_distance: SP / 2,
            skyline_clearance: SP / 4,
            ..PageSpec::default()
        };
        let mut systems = vec![plan(0, vec![0]), plan(12 * SP, vec![0])];
        let mut upper = staff_skyline();
        upper.add(SP, STAFF_HEIGHT, SP + 100, 14 * SP, STAFF_HEIGHT, 0);
        let skylines = vec![vec![upper], vec![staff_skyline()]];
        apply_skylines(&mut systems, &[skylines], &[], &spec);
        // 14 - 4 (staff height) + 0.5 (clearance) = 10.5 sp.
        assert_eq!(systems[1].y, 14 * SP + SP / 2);
    }

    #[test]
    fn without_spread_the_system_distance_is_capped() {
        // `pagelayout.cpp:561-602`: when spreading is off, free space is
        // shared up to `maxSystemDistance`, never beyond it.
        let spec = PageSpec {
            enable_vertical_spread: false,
            max_system_distance: 15 * SP,
            ..PageSpec::default()
        };
        let mut systems = vec![
            plan(0, vec![0]),
            plan(12 * SP, vec![0]),
            plan(24 * SP, vec![0]),
        ];
        for system in &mut systems {
            system.page = 0;
        }
        distribute_page(&mut systems, &[], &spec, 0);
        for pair in systems.windows(2) {
            assert!(pair[1].y - pair[0].y - pair[0].height <= spec.max_system_distance);
        }
        assert!(systems[1].y > 12 * SP);
    }

    #[test]
    fn systems_are_spread_down_the_page() {
        let spec = PageSpec::default();
        let mut systems: Vec<_> = (0..4).map(system).collect();
        for (index, item) in systems.iter_mut().enumerate() {
            item.y = 8 * SP + (index as i32) * 12 * SP;
        }
        spread_all(&mut systems, &[], &spec);
        let gaps: Vec<i32> = systems
            .windows(2)
            .map(|pair| pair[1].y - pair[0].y)
            .collect();
        assert!(gaps.iter().all(|gap| *gap >= 12 * SP));
        // A page of four systems is filled: the last one sits well below the
        // packed 8 + 3*12 = 44 sp position.
        assert!(systems.last().unwrap().y > 60 * SP);
    }

    #[test]
    fn staff_distances_are_spread_with_the_systems() {
        let spec = PageSpec::default();
        let mut systems = vec![system(0)];
        systems[0].staff_offsets = vec![0, 11 * SP, 22 * SP];
        let staffs = vec![
            StaffInfo {
                index: 0,
                part: 0,
                long_name: String::new(),
                short_name: String::new(),
                brackets: Vec::new(),
                bar_line_span: false,
            },
            StaffInfo {
                index: 1,
                part: 0,
                long_name: String::new(),
                short_name: String::new(),
                brackets: Vec::new(),
                bar_line_span: false,
            },
        ];
        spread_all(&mut systems, &staffs, &spec);
        assert!(systems[0].staff_offsets[1] >= 10 * SP);
        assert!(systems[0].staff_offsets[2] > systems[0].staff_offsets[1]);
        assert!(systems[0].height > 20 * SP);
    }
}
