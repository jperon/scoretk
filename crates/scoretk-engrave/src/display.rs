// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/tdraw.cpp:177` and
// `paint.cpp:38-70` of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright
// (C) MuseScore Limited and others, modified (rewritten in Rust) in 2026:
// the typed `EngravingItem` draw dispatch is replaced by one display-list item
// type with a kind, a SMuFL glyph and a rectangle.
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

//! Page-local display list, in fixed-point 1/1024 staff space.

use alloc::vec::Vec;

use scoretk_smufl::GLYPHS;

/// The role of an item, used by the SVG backend's class attribute and by the
/// geometric oracle to identify noteheads, stems and bar lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    StaffLine,
    BarLine,
    Stem,
    LedgerLine,
    Notehead,
    Rest,
    Clef,
    Accidental,
    TimeSig,
    Dot,
    Flag,
}

impl ItemKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaffLine => "StaffLines",
            Self::BarLine => "BarLine",
            Self::Stem => "Stem",
            Self::LedgerLine => "LedgerLine",
            Self::Notehead => "Note",
            Self::Rest => "Rest",
            Self::Clef => "Clef",
            Self::Accidental => "Accidental",
            Self::TimeSig => "TimeSig",
            Self::Dot => "NoteDot",
            Self::Flag => "Hook",
        }
    }
}

/// Geometry of one item: a SMuFL glyph, its quadratic outline, or a rectangle.
///
/// `Glyph` and `Path` both reference `scoretk-smufl::GLYPHS` by index; the
/// raster backend expands the stored quadratic edges, and the SVG backend can
/// either emit a `<path>` or reference a definition. Lines (staff, stems,
/// ledger, bar lines) are pixel-snapped rectangles, not paths (plan §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geometry {
    Glyph { glyph: u16, x: i32, y: i32 },
    Path { glyph: u16, x: i32, y: i32 },
    Rect { x: i32, y: i32, w: i32, h: i32 },
}

/// One drawable element of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayItem {
    pub kind: ItemKind,
    pub geometry: Geometry,
}

impl DisplayItem {
    pub const fn glyph(kind: ItemKind, glyph: u16, x: i32, y: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Glyph { glyph, x, y },
        }
    }

    pub const fn path(kind: ItemKind, glyph: u16, x: i32, y: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Path { glyph, x, y },
        }
    }

    pub const fn rect(kind: ItemKind, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Rect { x, y, w, h },
        }
    }

    /// Position of the item's origin in fixed-point staff spaces.
    pub const fn origin(&self) -> (i32, i32) {
        match self.geometry {
            Geometry::Glyph { x, y, .. } | Geometry::Path { x, y, .. } => (x, y),
            Geometry::Rect { x, y, .. } => (x, y),
        }
    }
}

/// The display list of one page; allocated in the caller's arena by the host
/// (the page cache of plan §4b is not part of this crate).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayList {
    pub page: usize,
    pub items: Vec<DisplayItem>,
}

impl DisplayList {
    pub fn new(page: usize) -> Self {
        Self {
            page,
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// Adds a glyph by SMuFL name; unknown names are ignored (the generated
    /// subset covers the notation families the engraver emits).
    pub fn glyph(&mut self, kind: ItemKind, name: &str, x: i32, y: i32) {
        if let Some(index) = scoretk_smufl::glyph_index(name) {
            self.push(DisplayItem::glyph(kind, index, x, y));
        }
    }

    /// Adds the quadratic outline of a glyph by SMuFL name.
    pub fn path(&mut self, kind: ItemKind, name: &str, x: i32, y: i32) {
        if let Some(index) = scoretk_smufl::glyph_index(name) {
            self.push(DisplayItem::path(kind, index, x, y));
        }
    }

    /// Number of items of a given kind, used by the oracle and tests.
    pub fn count(&self, kind: ItemKind) -> usize {
        self.items.iter().filter(|item| item.kind == kind).count()
    }

    /// Name of the SMuFL glyph referenced by an item, if any.
    pub fn glyph_name(&self, item: &DisplayItem) -> Option<&'static str> {
        match item.geometry {
            Geometry::Glyph { glyph, .. } | Geometry::Path { glyph, .. } => {
                GLYPHS.get(usize::from(glyph)).map(|g| g.name)
            }
            Geometry::Rect { .. } => None,
        }
    }
}
