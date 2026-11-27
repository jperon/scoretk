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

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::fmt::Write;
use alloc::string::String;
use alloc::vec::Vec;

use scoretk_engrave::display::{DisplayItem, DisplayList, Geometry, ItemKind};
use scoretk_engrave::layout::PageSpec;
use scoretk_smufl::{units_to_fixed, Edge, GLYPHS, TEXT_EDGES, UPEM};

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

/// Outline of one Edwin text glyph, scaled to the font em in fixed sp.
/// // Port of `tdraw.cpp`'s text drawing: the same y flip as music glyphs.
fn write_text_outline(out: &mut String, codepoint: u16, bold: bool, size: i32) {
    let Some(glyph) = scoretk_smufl::text_glyph(codepoint, bold) else {
        return;
    };
    let scale = |value: i16| i32::from(value) * size / i32::from(UPEM);
    let start = usize::from(glyph.edges.0);
    let end = start + usize::from(glyph.edges.1);
    let mut cx = 0i32;
    let mut cy = 0i32;
    for edge in &TEXT_EDGES[start..end] {
        match *edge {
            Edge::Move(x, y) => {
                cx = scale(x);
                cy = -scale(y);
                let _ = write!(out, "M{cx},{cy}");
            }
            Edge::Line(dx, dy) => {
                cx += scale(dx);
                cy -= scale(dy);
                let _ = write!(out, "L{cx},{cy}");
            }
            Edge::Quad(qx, qy, x, y) => {
                let control_x = cx + scale(qx);
                let control_y = cy - scale(qy);
                cx += scale(x);
                cy -= scale(y);
                let _ = write!(out, "Q{control_x},{control_y} {cx},{cy}");
            }
        }
    }
    let _ = write!(out, "Z");
}

/// Writes a 1/1024 fixed-point ratio as a decimal SVG number.
fn write_ratio(out: &mut String, value: i32) {
    let magnitude = value.unsigned_abs();
    let _ = write!(
        out,
        "{}{}.{:04}",
        if value < 0 { "-" } else { "" },
        magnitude / 1024,
        (magnitude % 1024) * 10000 / 1024
    );
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
        Geometry::Beam {
            x1,
            y1,
            x2,
            y2,
            width,
        } => {
            let _ = write!(
                out,
                "<path class=\"{class}\" d=\"M{x1},{y1} L{x2},{y2} \
                 L{x2},{bottom2} L{x1},{bottom1} Z\"/>",
                bottom1 = y1 + width,
                bottom2 = y2 + width,
            );
        }
        Geometry::Text {
            codepoint,
            x,
            y,
            size,
            bold,
        } => {
            let _ = write!(
                out,
                "<path class=\"{class}\" transform=\"translate({x},{y})\" d=\""
            );
            write_text_outline(out, codepoint, bold, size);
            out.push_str("\"/>");
        }
        Geometry::Scaled {
            glyph,
            x,
            y,
            sx,
            sy,
        } => {
            let _ = write!(
                out,
                "<path class=\"{class}\" transform=\"translate({x},{y}) scale("
            );
            write_ratio(out, sx);
            out.push(',');
            write_ratio(out, sy);
            let _ = write!(out, ")\" d=\"");
            // `write_outline` already flips SMuFL's y axis, so a positive
            // scale in both directions keeps the brace upright.
            write_outline(out, glyph);
            out.push_str("\"/>");
        }
        Geometry::Glyph { glyph, x, y, mag } | Geometry::Path { glyph, x, y, mag } => {
            if mag == 1024 {
                let _ = write!(
                    out,
                    "<path class=\"{class}\" transform=\"translate({x},{y})\" d=\""
                );
            } else {
                let _ = write!(
                    out,
                    "<path class=\"{class}\" transform=\"translate({x},{y}) scale("
                );
                write_ratio(out, mag);
                let _ = write!(out, ")\" d=\"");
            }
            write_outline(out, glyph);
            out.push_str("\"/>");
        }
    }
}

/// Class of the group wrapping a note-level item.
fn note_class(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Rest => "rest",
        ItemKind::Lyrics => "lyric",
        _ => "note",
    }
}

fn is_note_level(kind: ItemKind) -> bool {
    matches!(
        kind,
        ItemKind::Notehead
            | ItemKind::Rest
            | ItemKind::Stem
            | ItemKind::Accidental
            | ItemKind::Dot
            | ItemKind::Flag
            | ItemKind::Lyrics
    )
}

/// Writes items in **display-list order**, opening and closing the
/// system/measure/staff/note group stack around them. Since no item moves,
/// the painter order (and hence the rasterised pixels) is unchanged.
fn write_grouped_items(out: &mut String, list: &DisplayList, items: &[&DisplayItem]) {
    let mut measure: Option<(usize, usize)> = None; // (ordinal, part)
    let mut staff: Option<usize> = None;
    let mut note: Option<(String, &'static str)> = None;
    let mut emitted_measures = BTreeSet::new();
    let mut emitted_notes = BTreeSet::new();
    for item in items {
        let new_measure = item.source.measure_ordinal().zip(item.source.part());
        let new_staff = item.source.staff();
        let new_note = if is_note_level(item.kind) {
            item.source
                .xml_id(&list.sources)
                .map(|id| (id, note_class(item.kind)))
        } else {
            None
        };
        if new_measure != measure {
            if note.is_some() {
                out.push_str("</g>");
                note = None;
            }
            if staff.is_some() {
                out.push_str("</g>");
                staff = None;
            }
            if measure.is_some() {
                out.push_str("</g>");
            }
            measure = new_measure;
            if let Some((ordinal, part)) = new_measure {
                // Some parts repeat a measure group across staves: the
                // `id` appears only on the first group, XML ids stay unique.
                if emitted_measures.insert((part, ordinal)) {
                    let _ = write!(
                        out,
                        "<g class=\"measure\" id=\"p{part}-m{ordinal}\" data-ordinal=\"{ordinal}\">"
                    );
                } else {
                    let _ = write!(out, "<g class=\"measure\" data-ordinal=\"{ordinal}\">");
                }
            }
        }
        if new_staff != staff {
            if note.is_some() {
                out.push_str("</g>");
                note = None;
            }
            if staff.is_some() {
                out.push_str("</g>");
            }
            staff = new_staff;
            if let Some(value) = new_staff {
                let _ = write!(out, "<g class=\"staff\" data-staff=\"{value}\">");
            }
        }
        if new_note != note {
            if note.is_some() {
                out.push_str("</g>");
            }
            note = new_note.clone();
            if let Some((id, class)) = &new_note {
                if emitted_notes.insert(id.clone()) {
                    let _ = write!(out, "<g class=\"{class}\" id=\"{id}\" data-note=\"{id}\">");
                } else {
                    let _ = write!(out, "<g class=\"{class}\" data-note=\"{id}\">");
                }
            }
        }
        write_item(out, item);
    }
    if note.is_some() {
        out.push_str("</g>");
    }
    if staff.is_some() {
        out.push_str("</g>");
    }
    if measure.is_some() {
        out.push_str("</g>");
    }
}

/// Renders one page. The output keeps one path/rect per display item so the
/// geometric oracle can extract noteheads, stems and bar lines; groups and
/// identifiers are added around them without changing any coordinate.
pub fn render_page(list: &DisplayList, spec: &PageSpec) -> String {
    let width = spec.printable_width + 2 * spec.margin_left;
    let height = spec.page_height;
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
    // Group without reordering any item inside a (system, measure, staff)
    // bucket: the paths/rects and their coordinates stay byte-identical.
    let mut systems: BTreeMap<usize, Vec<&DisplayItem>> = BTreeMap::new();
    for item in &list.items {
        match item.source.system_index() {
            Some(system) => systems.entry(system).or_default().push(item),
            None => write_item(&mut out, item),
        }
    }
    for (system, items) in systems {
        let _ = write!(out, "<g class=\"system\" data-system=\"{system}\">");
        write_grouped_items(&mut out, list, &items);
        out.push_str("</g>");
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

    #[test]
    fn structured_groups_carry_stable_identifiers() {
        let mut list = DisplayList::new(0);
        list.set_system(2);
        list.set_staff(2, 1, 0);
        list.push(DisplayItem::rect(ItemKind::StaffLine, 0, 0, 100, 1));
        list.set_measure(7, 0);
        list.set_note(Some(scoretk_engrave::NoteIdentity::note(7, 1, 0, 4, 0)));
        list.path(ItemKind::Notehead, "noteheadBlack", 30, 40);
        list.set_note(None);
        list.set_note(Some(scoretk_engrave::NoteIdentity::note(7, 1, 0, 5, 0)));
        list.push(DisplayItem::rect(ItemKind::Rest, 50, 20, 10, 10));
        let svg = render_page(&list, &PageSpec::default());
        assert!(svg.contains("<g class=\"system\" data-system=\"2\">"));
        assert!(svg.contains("<g class=\"measure\" id=\"p0-m7\" data-ordinal=\"7\">"));
        assert!(svg.contains("<g class=\"staff\" data-staff=\"1\">"));
        assert!(svg.contains("<g class=\"note\" id=\"m7-s1-v0-c4-n0\""));
        assert!(svg.contains("<g class=\"rest\" id=\"m7-s1-v0-c5-n0\""));
    }
}
