// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/infrastructure/skyline.{h,cpp}:40-91, 117-143,
// 192-226` and `src/engraving/rendering/score/systemlayout.cpp:2485-2581,
// 2892-2989` of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright (C) MuseScore
// Limited and others, modified (rewritten in Rust) in 2026: each skyline is
// a plain list of x-intervals with y-extents, with no `EngravingItem*`,
// `ShapeElement` or `System*`/`SysStaff*` object graph.
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

//! Per-staff north/south skylines of the items of one system.
//!
//! Coordinates are fixed 1/1024 staff spaces, relative to the staff's own
//! top line: `north` holds the highest ink of every item poking above the
//! staff, `south` the lowest ink of every item poking below it
//! (`skyline.cpp:117-143`). Both include the staff lines themselves, so the
//! distance between two skylines already contains the height of the upper
//! staff.

use alloc::vec::Vec;

/// The ink interval of one drawn item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Interval {
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
}

/// One `SkylineLine` (`skyline.h:44-96`), without the item pointer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkylineLine {
    intervals: Vec<Interval>,
    north: bool,
}

impl SkylineLine {
    pub fn new(north: bool) -> Self {
        Self {
            intervals: Vec::new(),
            north,
        }
    }

    /// // Port of `SkylineLine::add(const ShapeElement&)`
    /// (`skyline.cpp:117-143`): an item entirely inside the staff lines does
    /// not poke out and is only kept on the side it cannot reach. Everything
    /// else is added to both sides, where `max()` reduces it to the extreme.
    pub fn add(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, staff_height: i32) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        if self.north && y0 >= 0 {
            return;
        }
        if !self.north && y1 <= staff_height {
            return;
        }
        self.push(x0, y0, x1, y1);
    }

    /// Staff lines always contribute, even though their extent is exactly
    /// the staff (`skyline.cpp:129-140`): they define the base distance.
    pub fn push(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        self.intervals.push(Interval { x0, x1, y0, y1 });
    }

    /// `SkylineLine::max()` (`skyline.cpp:306-309`): the most extreme y,
    /// measured from the staff's top line (negative above, ≥ staff height
    /// below).
    pub fn max(&self) -> i32 {
        let mut extreme = if self.north { i32::MAX } else { i32::MIN };
        for interval in &self.intervals {
            extreme = if self.north {
                extreme.min(interval.y0)
            } else {
                extreme.max(interval.y1)
            };
        }
        if extreme == i32::MAX || extreme == i32::MIN {
            0
        } else {
            extreme
        }
    }

    /// // Port of `Skyline::minDistance` (`skyline.cpp:198-206`) via
    /// `Shape::minVerticalDistance` (`shape.cpp:221-245`): over all item pairs
    /// that overlap horizontally (with `skylineMinHorizontalClearance`), take
    /// the **largest** `upper.bottom - lower.top`. A positive value means the
    /// two outlines already overlap. `None` means there is no horizontal
    /// overlap at all, in which case the style distance alone applies.
    pub fn min_distance(&self, lower: &Self, clearance: i32) -> Option<i32> {
        let mut distance: Option<i32> = None;
        for upper in &self.intervals {
            for lower in &lower.intervals {
                if upper.y1 <= upper.y0 || lower.y1 <= lower.y0 {
                    continue;
                }
                // `intersects(ax1, ax2, bx1, bx2, minClearance)` of
                // `shape.h:192-198`.
                if upper.x1 + clearance <= lower.x0 || upper.x0 >= lower.x1 + clearance {
                    continue;
                }
                let gap = upper.y1 - lower.y0;
                distance = Some(distance.map_or(gap, |current| current.max(gap)));
            }
        }
        distance
    }

    /// Lowest ink within an x range, used by `LyricsLayout::
    /// checkCollisionsWithStaffElements` (`lyricslayout.cpp:616-652`) to keep
    /// a lyric line clear of the staff's own outline.
    pub fn max_at(&self, x0: i32, x1: i32) -> Option<i32> {
        let mut lowest: Option<i32> = None;
        for interval in &self.intervals {
            if interval.x1 > x0 && interval.x0 < x1 {
                lowest = Some(lowest.map_or(interval.y1, |value| value.max(interval.y1)));
            }
        }
        lowest
    }

    /// Number of contributing intervals, for diagnostics and tests.
    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }
}

/// The two skylines of one staff (`skyline.h:102-126`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skyline {
    north: SkylineLine,
    south: SkylineLine,
}

impl Skyline {
    pub fn new() -> Self {
        Self {
            north: SkylineLine::new(true),
            south: SkylineLine::new(false),
        }
    }

    /// Adds the staff lines themselves to both sides (they define the base
    /// distance even though they do not poke outside the staff).
    pub fn add_staff_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.north.push(x0, y0, x1, y1);
        self.south.push(x0, y0, x1, y1);
    }

    /// Adds one drawn item in staff-local coordinates. The padding extends
    /// the south side of lyric items by `lyricsMinBottomDistance -
    /// minVerticalDistance` (`lyricslayout.cpp:704-742`): the clearance is
    /// added once here, not again when the next staff is placed.
    pub fn add(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, staff_height: i32, south_pad: i32) {
        self.north.add(x0, y0, x1, y1, staff_height);
        self.south
            .add(x0, y0, x1, y1 + south_pad.max(0), staff_height);
    }

    /// Adds the intervals of another skyline, used for the decorations drawn
    /// before a system's first staff.
    pub fn merge(&mut self, other: Skyline) {
        self.north.intervals.extend(other.north.intervals);
        self.south.intervals.extend(other.south.intervals);
    }

    pub fn north(&self) -> &SkylineLine {
        &self.north
    }

    pub fn south(&self) -> &SkylineLine {
        &self.south
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::SP;

    #[test]
    fn staff_lines_define_the_base_distance() {
        let mut upper = Skyline::new();
        let mut lower = Skyline::new();
        upper.add_staff_line(0, 0, 10 * SP, 4 * SP);
        lower.add_staff_line(0, 0, 10 * SP, 4 * SP);
        assert_eq!(upper.north().max(), 0);
        assert_eq!(upper.south().max(), 4 * SP);
        assert_eq!(
            upper.south().min_distance(lower.north(), SP / 4),
            Some(4 * SP)
        );
    }

    #[test]
    fn a_stem_below_the_staff_pushes_the_next_staff() {
        let mut upper = Skyline::new();
        let mut lower = Skyline::new();
        upper.add_staff_line(0, 0, 10 * SP, 4 * SP);
        upper.add(3 * SP, 4 * SP, 3 * SP + 100, 8 * SP, 4 * SP, 0);
        lower.add_staff_line(0, 0, 10 * SP, 4 * SP);
        assert_eq!(upper.south().max(), 8 * SP);
        assert_eq!(
            upper.south().min_distance(lower.north(), SP / 4),
            Some(8 * SP)
        );
    }

    #[test]
    fn non_overlapping_x_intervals_do_not_constrain() {
        let mut upper = Skyline::new();
        let mut lower = Skyline::new();
        upper.add_staff_line(0, 0, SP, 4 * SP);
        lower.add_staff_line(4 * SP, 0, 5 * SP, 4 * SP);
        assert_eq!(upper.south().min_distance(lower.north(), SP / 4), None);
        // A stem reaching below the upper staff at the lower staff's x now
        // constrains the distance (the clearance is added by the caller).
        upper.add(4 * SP, 4 * SP, 4 * SP + 100, 9 * SP, 4 * SP, 0);
        assert_eq!(
            upper.south().min_distance(lower.north(), SP / 4),
            Some(9 * SP)
        );
    }
}
