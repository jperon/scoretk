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
use scoretk_smufl::{text_glyph, units_to_fixed, Edge, GLYPHS, TEXT_EDGES, UPEM};

use crate::{Gray8, Path, Point, ONE};

/// Converts a layout coordinate (1/1024 sp) to rasteriser units (1/256 px).
fn to_pixels(fixed: i32, staff_px: i32) -> i32 {
    (i64::from(fixed) * i64::from(staff_px) * i64::from(ONE) / 1024) as i32
}

/// Rounds a rasteriser coordinate (1/256 px) to a whole pixel.
///
/// `Gray8::fill_rect` takes integer pixels, unlike `fill_path` which takes
/// the fixed-point `Point`s produced by `to_pixels`. Rects (staff lines,
/// stems, bar lines, ledger lines) must therefore be converted here, or
/// their 1/256-px coordinates land far outside the framebuffer and are
/// entirely clipped by `fill_rect`.
fn to_whole_pixels(units: i32) -> i32 {
    (units + ONE / 2) / ONE
}

/// Draws one display item into `buf`, translated by `(origin_x, origin_y)`
/// pixels. `ink` is the grey value (0 = black, 255 = white).
fn draw_item(buf: &mut Gray8<'_>, item: &DisplayItem, origin: Point, staff_px: i32, ink: u8) {
    match item.geometry {
        Geometry::Rect { x, y, w, h } => {
            buf.fill_rect(
                to_whole_pixels(origin.x + to_pixels(x, staff_px)),
                to_whole_pixels(origin.y + to_pixels(y, staff_px)),
                to_whole_pixels(to_pixels(w, staff_px)).max(1),
                to_whole_pixels(to_pixels(h, staff_px)).max(1),
                ink,
            );
        }
        Geometry::Beam {
            x1,
            y1,
            x2,
            y2,
            width,
        } => {
            let mut path = Path::new();
            let points = [
                Point::new(
                    origin.x + to_pixels(x1, staff_px),
                    origin.y + to_pixels(y1, staff_px),
                ),
                Point::new(
                    origin.x + to_pixels(x2, staff_px),
                    origin.y + to_pixels(y2, staff_px),
                ),
                Point::new(
                    origin.x + to_pixels(x2, staff_px),
                    origin.y + to_pixels(y2 + width, staff_px),
                ),
                Point::new(
                    origin.x + to_pixels(x1, staff_px),
                    origin.y + to_pixels(y1 + width, staff_px),
                ),
            ];
            path.move_to(points[0]);
            for point in &points[1..] {
                path.line_to(*point);
            }
            path.close();
            buf.fill_path(&path, ink);
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
                    // // Port of `tdraw.cpp:2595-2630`: SMuFL y is
                    // up-positive, the framebuffer y is down-positive.
                    Edge::Move(x, y) => {
                        cursor = Point::new(
                            base.x + to_pixels(units_to_fixed(x), staff_px),
                            base.y - to_pixels(units_to_fixed(y), staff_px),
                        );
                        path.move_to(cursor);
                    }
                    Edge::Line(dx, dy) => {
                        let next = Point::new(
                            cursor.x + to_pixels(units_to_fixed(dx), staff_px),
                            cursor.y - to_pixels(units_to_fixed(dy), staff_px),
                        );
                        path.line_to(next);
                        cursor = next;
                    }
                    Edge::Quad(qx, qy, x, y) => {
                        let control = Point::new(
                            cursor.x + to_pixels(units_to_fixed(qx), staff_px),
                            cursor.y - to_pixels(units_to_fixed(qy), staff_px),
                        );
                        let next = Point::new(
                            cursor.x + to_pixels(units_to_fixed(x), staff_px),
                            cursor.y - to_pixels(units_to_fixed(y), staff_px),
                        );
                        path.quad_to(control, next);
                        cursor = next;
                    }
                }
            }
            path.close();
            buf.fill_path(&path, ink);
        }
        Geometry::Scaled {
            glyph,
            x,
            y,
            sx,
            sy,
        } => {
            let Some(g) = GLYPHS.get(usize::from(glyph)) else {
                return;
            };
            let base = Point::new(
                origin.x + to_pixels(x, staff_px),
                origin.y + to_pixels(y, staff_px),
            );
            let scale = |value: i32, factor: i32| value * factor / 1024;
            let mut path = Path::new();
            let mut cursor = base;
            let start = usize::from(g.edges.0);
            let end = start + usize::from(g.edges.1);
            for edge in &scoretk_smufl::EDGES[start..end] {
                match *edge {
                    Edge::Move(gx, gy) => {
                        cursor = Point::new(
                            base.x + to_pixels(scale(i32::from(gx), sx), staff_px),
                            base.y - to_pixels(scale(i32::from(gy), sy), staff_px),
                        );
                        path.move_to(cursor);
                    }
                    Edge::Line(dx, dy) => {
                        let next = Point::new(
                            cursor.x + to_pixels(scale(i32::from(dx), sx), staff_px),
                            cursor.y - to_pixels(scale(i32::from(dy), sy), staff_px),
                        );
                        path.line_to(next);
                        cursor = next;
                    }
                    Edge::Quad(qx, qy, x, y) => {
                        let control = Point::new(
                            cursor.x + to_pixels(scale(i32::from(qx), sx), staff_px),
                            cursor.y - to_pixels(scale(i32::from(qy), sy), staff_px),
                        );
                        let next = Point::new(
                            cursor.x + to_pixels(scale(i32::from(x), sx), staff_px),
                            cursor.y - to_pixels(scale(i32::from(y), sy), staff_px),
                        );
                        path.quad_to(control, next);
                        cursor = next;
                    }
                }
            }
            path.close();
            buf.fill_path(&path, ink);
        }
        Geometry::Text {
            codepoint,
            x,
            y,
            size,
            bold,
        } => {
            let Some(glyph) = text_glyph(codepoint, bold) else {
                return;
            };
            let base = Point::new(
                origin.x + to_pixels(x, staff_px),
                origin.y + to_pixels(y, staff_px),
            );
            let scale = |value: i16| i32::from(value) * size / i32::from(UPEM);
            let mut path = Path::new();
            let mut cursor = base;
            let start = usize::from(glyph.edges.0);
            let end = start + usize::from(glyph.edges.1);
            for edge in &TEXT_EDGES[start..end] {
                match *edge {
                    Edge::Move(x, y) => {
                        cursor = Point::new(
                            base.x + to_pixels(scale(x), staff_px),
                            base.y - to_pixels(scale(y), staff_px),
                        );
                        path.move_to(cursor);
                    }
                    Edge::Line(dx, dy) => {
                        let next = Point::new(
                            cursor.x + to_pixels(scale(dx), staff_px),
                            cursor.y - to_pixels(scale(dy), staff_px),
                        );
                        path.line_to(next);
                        cursor = next;
                    }
                    Edge::Quad(qx, qy, x, y) => {
                        let control = Point::new(
                            cursor.x + to_pixels(scale(qx), staff_px),
                            cursor.y - to_pixels(scale(qy), staff_px),
                        );
                        let next = Point::new(
                            cursor.x + to_pixels(scale(x), staff_px),
                            cursor.y - to_pixels(scale(y), staff_px),
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
    fn rectangles_are_in_whole_pixels_not_fixed_point_units() {
        let mut list = DisplayList::new(0);
        // One staff space is 1024 layout units = 14 px at this staff size.
        // `fill_rect` works in whole pixels: a rect at (1 sp, 2 sp), 3 sp
        // wide, translated by (1, 2) px must land at x=15..57, y=30..32.
        list.push(DisplayItem::rect(
            ItemKind::StaffLine,
            1024,
            2048,
            3 * 1024,
            113,
        ));
        let mut pixels = [255u8; 64 * 64];
        let mut buf = Gray8::new(&mut pixels, 64, 64).unwrap();
        draw_display_list(&list, &mut buf, Point::from_pixels(1, 2), 14);
        assert_eq!(pixels[30 * 64 + 14], 255, "left of the rectangle");
        assert_eq!(pixels[30 * 64 + 15], 0, "rectangle starts at x=15");
        assert_eq!(pixels[30 * 64 + 56], 0, "rectangle ends at x=56");
        assert_eq!(pixels[30 * 64 + 57], 255, "right of the rectangle");
        assert_eq!(pixels[29 * 64 + 20], 255, "above the rectangle");
        assert_eq!(pixels[30 * 64 + 20], 0, "rectangle covers row 30");
        assert_eq!(pixels[32 * 64 + 20], 255, "below the rectangle");
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
