// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/tdraw.cpp:177-330` and
// `paint.cpp:38-70` of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright
// (C) MuseScore Limited and others, modified (rewritten in Rust) in 2026:
// one `<path>`/`<rect>` per display-list item instead of a typed
// `EngravingItem` dispatch over the whole document. The SVG structure and
// glyph deduplication idea follow Verovio's `svgdevicecontext.cpp:103-128`
// (LGPL-3.0-or-later, architecture only, no code copied).
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

//! SVG backend for `scoretk-engrave`'s display list. `no_std` + `alloc`.
//!
//! Coordinates in the SVG are fixed-point units of 1/1024 staff space, so
//! the geometry oracle can compare them to MuseScore's SVG in staff spaces
//! without any floating-point rounding in the layout stage.

#![no_std]

extern crate alloc;

use alloc::fmt::Write;
use alloc::string::String;

use scoretk_engrave::display::{DisplayItem, DisplayList, Geometry, ItemKind};
use scoretk_engrave::layout::PageSpec;
use scoretk_smufl::{units_to_fixed, Edge, GLYPHS};

/// Appends the quadratic outline of a SMuFL glyph, in fixed 1/1024 sp units.
///
/// // Port of `tdraw.cpp:2595-2630`: SMuFL glyph coordinates and anchors have
/// y increasing upwards, while MuseScore's painter (and our SVG viewBox) has
/// y increasing downwards. Negating y here keeps the glyph upright.
fn write_outline(out: &mut String, glyph_index: u16) {
    let Some(glyph) = GLYPHS.get(usize::from(glyph_index)) else {
        return;
    };
    let start = usize::from(glyph.edges.0);
    let end = start + usize::from(glyph.edges.1);
    let mut cx = 0i32;
    let mut cy = 0i32;
    for edge in &scoretk_smufl::EDGES[start..end] {
        match *edge {
            Edge::Move(x, y) => {
                cx = units_to_fixed(x);
                cy = -units_to_fixed(y);
                let _ = write!(out, "M{cx},{cy}");
            }
            Edge::Line(dx, dy) => {
                cx += units_to_fixed(dx);
                cy -= units_to_fixed(dy);
                let _ = write!(out, "L{cx},{cy}");
            }
            Edge::Quad(qx, qy, x, y) => {
                let control_x = cx + units_to_fixed(qx);
                let control_y = cy - units_to_fixed(qy);
                cx += units_to_fixed(x);
                cy -= units_to_fixed(y);
                let _ = write!(out, "Q{control_x},{control_y} {cx},{cy}");
            }
        }
    }
    let _ = write!(out, "Z");
}

fn write_item(out: &mut String, item: &DisplayItem) {
    let class = item.kind.as_str();
    match item.geometry {
        Geometry::Rect { x, y, w, h } => {
            let _ = write!(
                out,
                "<rect class=\"{class}\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>"
            );
        }
        Geometry::Glyph { glyph, x, y } | Geometry::Path { glyph, x, y } => {
            let _ = write!(
                out,
                "<path class=\"{class}\" transform=\"translate({x},{y})\" d=\""
            );
            write_outline(out, glyph);
            out.push_str("\"/>");
        }
    }
}

/// Renders one page. The output keeps one element per display item so the
/// geometric oracle can extract noteheads, stems and bar lines.
pub fn render_page(list: &DisplayList, spec: &PageSpec) -> String {
    let width = spec.printable_width + 2 * spec.margin_left;
    let height = 2 * spec.margin_top + spec.systems_per_page as i32 * 12 * 1024;
    let width_mm = PageSpec::fixed_to_mm(width);
    let height_mm = PageSpec::fixed_to_mm(height);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width_mm:.3}mm\" \
         height=\"{height_mm:.3}mm\" viewBox=\"0 0 {width} {height}\">"
    );
    out.push('\n');
    let _ = write!(
        out,
        "<rect class=\"Page\" x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"#fff\"/>"
    );
    for item in &list.items {
        write_item(&mut out, item);
    }
    out.push_str("\n</svg>\n");
    out
}

/// Renders all pages of an engraver into one SVG string per page.
pub fn render_all(engraver: &scoretk_engrave::Engraver) -> alloc::vec::Vec<String> {
    let mut pages = alloc::vec::Vec::with_capacity(engraver.page_count());
    for page in 0..engraver.page_count() {
        pages.push(render_page(&engraver.page(page), engraver.spec()));
    }
    pages
}

/// Convenience re-export for the oracle's class names.
pub const fn oracle_class(kind: ItemKind) -> &'static str {
    kind.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_engrave::layout::PageSpec;

    fn sample_list() -> DisplayList {
        let mut list = DisplayList::new(0);
        list.push(DisplayItem::rect(ItemKind::StaffLine, 10, 20, 100, 1));
        list.path(ItemKind::Notehead, "noteheadBlack", 30, 40);
        list
    }

    #[test]
    fn svg_has_one_element_per_display_item() {
        let svg = render_page(&sample_list(), &PageSpec::default());
        assert!(svg.contains("class=\"StaffLines\""));
        assert!(svg.contains("class=\"Note\""));
        assert!(svg.contains("<path"));
        assert!(svg.contains("<rect"));
        assert!(svg.contains("viewBox=\"0 0 "));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn notehead_uses_quadratic_commands() {
        let svg = render_page(&sample_list(), &PageSpec::default());
        let note = svg.split("class=\"Note\"").nth(1).unwrap();
        assert!(
            note.contains('Q'),
            "notehead should have quadratic segments"
        );
    }

    #[test]
    fn bar_lines_are_rectangles_not_paths() {
        let mut list = sample_list();
        list.push(DisplayItem::rect(ItemKind::BarLine, 50, 20, 1, 40));
        let svg = render_page(&list, &PageSpec::default());
        let bar = svg.split("class=\"BarLine\"").nth(1).unwrap();
        assert!(bar.starts_with(" x="));
    }
}
