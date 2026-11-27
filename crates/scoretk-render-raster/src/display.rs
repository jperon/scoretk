// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/tdraw.cpp:177-330` of MuseScore
// Studio 4.7.4 (tag v4.7.4), Copyright (C) MuseScore Limited and others,
// modified (rewritten in Rust) in 2026: the typed painter dispatch becomes a
// walk over the display list. The scanline fill is ours (P0 spike).
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

//! Draws a `scoretk-engrave` display list into a caller-owned `Gray8` buffer.
//!
//! Layout coordinates are 1/1024 staff space; the rasteriser works in 1/256
//! pixel. `staff_px` is the staff height in pixels (14 on the Kobo at ~200
//! ppi). Lines are snapped to the pixel grid, glyphs use the generated Leland
//! quadratic outlines.

use scoretk_engrave::display::{DisplayItem, DisplayList, Geometry};
use scoretk_smufl::{units_to_fixed, Edge, GLYPHS};

use crate::{Gray8, Path, Point, ONE};

/// Converts a layout coordinate (1/1024 sp) to rasteriser units (1/256 px).
fn to_pixels(fixed: i32, staff_px: i32) -> i32 {
    (i64::from(fixed) * i64::from(staff_px) * i64::from(ONE) / 1024) as i32
}

/// Draws one display item into `buf`, translated by `(origin_x, origin_y)`
/// pixels. `ink` is the grey value (0 = black, 255 = white).
fn draw_item(buf: &mut Gray8<'_>, item: &DisplayItem, origin: Point, staff_px: i32, ink: u8) {
    match item.geometry {
        Geometry::Rect { x, y, w, h } => {
            buf.fill_rect(
                origin.x + to_pixels(x, staff_px),
                origin.y + to_pixels(y, staff_px),
                to_pixels(w, staff_px).max(1),
                to_pixels(h, staff_px).max(1),
                ink,
            );
        }
        Geometry::Glyph { glyph, x, y } | Geometry::Path { glyph, x, y } => {
            let Some(g) = GLYPHS.get(usize::from(glyph)) else {
                return;
            };
            let base = Point::new(
                origin.x + to_pixels(x, staff_px),
                origin.y + to_pixels(y, staff_px),
            );
            let mut path = Path::new();
            let mut cursor = base;
            let start = usize::from(g.edges.0);
            let end = start + usize::from(g.edges.1);
            for edge in &scoretk_smufl::EDGES[start..end] {
                match *edge {
                    Edge::Move(x, y) => {
                        cursor = Point::new(
                            base.x + to_pixels(units_to_fixed(x), staff_px),
                            base.y + to_pixels(units_to_fixed(y), staff_px),
                        );
                        path.move_to(cursor);
                    }
                    Edge::Line(dx, dy) => {
                        let next = Point::new(
                            cursor.x + to_pixels(units_to_fixed(dx), staff_px),
                            cursor.y + to_pixels(units_to_fixed(dy), staff_px),
                        );
                        path.line_to(next);
                        cursor = next;
                    }
                    Edge::Quad(qx, qy, x, y) => {
                        let control = Point::new(
                            cursor.x + to_pixels(units_to_fixed(qx), staff_px),
                            cursor.y + to_pixels(units_to_fixed(qy), staff_px),
                        );
                        let next = Point::new(
                            cursor.x + to_pixels(units_to_fixed(x), staff_px),
                            cursor.y + to_pixels(units_to_fixed(y), staff_px),
                        );
                        path.quad_to(control, next);
                        cursor = next;
                    }
                }
            }
            path.close();
            buf.fill_path(&path, ink);
        }
    }
}

/// Draws a full page into `buf` (255 = white background, 0 = black ink).
pub fn draw_display_list(list: &DisplayList, buf: &mut Gray8<'_>, origin: Point, staff_px: i32) {
    buf.clear(255);
    for item in &list.items {
        draw_item(buf, item, origin, staff_px, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_engrave::display::{DisplayItem, DisplayList, ItemKind};

    #[test]
    fn draws_rectangles_and_glyphs() {
        let mut list = DisplayList::new(0);
        list.push(DisplayItem::rect(
            ItemKind::StaffLine,
            0,
            0,
            100 * 1024,
            113,
        ));
        list.path(ItemKind::Notehead, "noteheadBlack", 8 * 1024, 1024);
        let mut pixels = [255u8; 64 * 32];
        let mut buf = Gray8::new(&mut pixels, 64, 32).unwrap();
        draw_display_list(&list, &mut buf, Point::new(0, 0), 14);
        assert!(buf.as_slice().iter().any(|value| *value < 128));
    }

    #[test]
    fn an_empty_list_is_a_white_page() {
        let list = DisplayList::new(0);
        let mut pixels = [0u8; 16 * 16];
        let mut buf = Gray8::new(&mut pixels, 16, 16).unwrap();
        draw_display_list(&list, &mut buf, Point::new(0, 0), 14);
        assert!(buf.as_slice().iter().all(|value| *value == 255));
    }
}
