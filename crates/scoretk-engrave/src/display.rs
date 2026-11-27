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

use scoretk_smufl::{units_to_fixed, GLYPHS};

use crate::identity::{NoteIdentity, NoteSourceRange, SourceRef};

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
    Beam,
    /// Title, subtitle, measure numbers and other Edwin text.
    Text,
    /// One Edwin glyph of a lyric syllable or its hyphen/melisma line. Kept
    /// apart from other text for the south-skyline padding of
    /// `LyricsLayout::addToSkyline` (`lyricslayout.cpp:704-742`).
    Lyrics,
    /// Square choir bracket or curved piano brace.
    Bracket,
    /// Short or long instrument name at the left of a system.
    InstrumentName,
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
            Self::Beam => "Beam",
            Self::Text => "Text",
            Self::Lyrics => "Lyrics",
            Self::Bracket => "Bracket",
            Self::InstrumentName => "InstrumentName",
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
    Glyph {
        glyph: u16,
        x: i32,
        y: i32,
        /// Staff magnitude in 1/1024 (`EngravingItem::mag`, 1024 = 1.0).
        /// Only the outline is scaled: the segment x is shared by all staves.
        mag: i32,
    },
    Path {
        glyph: u16,
        x: i32,
        y: i32,
        mag: i32,
    },
    Rect {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// A slanted beam: the top edge runs `(x1,y1) → (x2,y2)`, the bottom
    /// edge is `beamWidth` lower (`beamlayout.cpp:1215-1216`,
    /// `createBeamSegment`). // Port of `tdraw.cpp`'s beam polygon.
    Beam {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        width: i32,
    },
    /// One Edwin glyph of a text run: `codepoint` indexes the generated
    /// Latin subset in `scoretk-smufl`, `size` is the font em in fixed staff
    /// spaces (1/1024), and `x`/`y` are the glyph's pen origin and baseline.
    Text {
        codepoint: u16,
        x: i32,
        y: i32,
        size: i32,
        bold: bool,
    },
    /// A SMuFL glyph stretched to a given box, used for the piano brace
    /// (`tdraw.cpp:1088-1105` scales the `brace` symbol to the bracket
    /// height). `x`/`y` are the translation of the font outline and
    /// `sx`/`sy` are 1/1024 scale factors.
    Scaled {
        glyph: u16,
        x: i32,
        y: i32,
        sx: i32,
        sy: i32,
    },
}

/// One drawable element of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayItem {
    pub kind: ItemKind,
    pub geometry: Geometry,
    /// Compact reference to the source element: system, measure ordinal,
    /// staff, part and, for note-level items, the stable note identity.
    /// Stamped by `DisplayList::push` from the list's current context.
    pub source: SourceRef,
}

impl DisplayItem {
    pub const fn glyph(kind: ItemKind, glyph: u16, x: i32, y: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Glyph {
                glyph,
                x,
                y,
                mag: 1024,
            },
            source: SourceRef::none(),
        }
    }

    pub const fn path(kind: ItemKind, glyph: u16, x: i32, y: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Path {
                glyph,
                x,
                y,
                mag: 1024,
            },
            source: SourceRef::none(),
        }
    }

    /// A glyph drawn at a non-unit staff magnitude. Port of
    /// `EngravingItem::mag()` (`engravingitem.cpp:2911-2918`) and
    /// `Staff::staffMag` (`staff.cpp:991-1004`): only the outline scales,
    /// the shared segment x does not.
    pub const fn glyph_mag(kind: ItemKind, glyph: u16, x: i32, y: i32, mag: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Glyph { glyph, x, y, mag },
            source: SourceRef::none(),
        }
    }

    /// Quadratic outline at a non-unit staff magnitude.
    pub const fn path_mag(kind: ItemKind, glyph: u16, x: i32, y: i32, mag: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Path { glyph, x, y, mag },
            source: SourceRef::none(),
        }
    }

    pub const fn rect(kind: ItemKind, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Rect { x, y, w, h },
            source: SourceRef::none(),
        }
    }

    pub const fn beam(kind: ItemKind, x1: i32, y1: i32, x2: i32, y2: i32, width: i32) -> Self {
        Self {
            kind,
            geometry: Geometry::Beam {
                x1,
                y1,
                x2,
                y2,
                width,
            },
            source: SourceRef::none(),
        }
    }

    pub const fn text(
        kind: ItemKind,
        codepoint: u16,
        x: i32,
        y: i32,
        size: i32,
        bold: bool,
    ) -> Self {
        Self {
            kind,
            geometry: Geometry::Text {
                codepoint,
                x,
                y,
                size,
                bold,
            },
            source: SourceRef::none(),
        }
    }

    pub const fn scaled(glyph: u16, x: i32, y: i32, sx: i32, sy: i32) -> Self {
        Self {
            kind: ItemKind::Bracket,
            geometry: Geometry::Scaled {
                glyph,
                x,
                y,
                sx,
                sy,
            },
            source: SourceRef::none(),
        }
    }

    /// Moves one drawn item down by `dy`, used by lyric autoplace.
    pub fn translate_y(&mut self, dy: i32) {
        match &mut self.geometry {
            Geometry::Glyph { y, .. }
            | Geometry::Path { y, .. }
            | Geometry::Text { y, .. }
            | Geometry::Scaled { y, .. }
            | Geometry::Rect { y, .. } => *y += dy,
            Geometry::Beam { y1, y2, .. } => {
                *y1 += dy;
                *y2 += dy;
            }
        }
    }

    /// Scales one item to a staff's magnitude around its staff's top line.
    ///
    /// // Port of `EngravingItem::mag()` (`engravingitem.cpp:2911-2918`) and
    /// `Staff::staffMag` (`staff.cpp:991-1004`): the staff spatium is the
    /// score spatium times `mag`, so glyph outlines, stems, ledger lines,
    /// lyric text and inter-staff distances all scale. Horizontal segment
    /// positions are shared by every staff of a system and are left alone.
    /// `mag == 1024` is a no-op, so the default output is byte-identical.
    pub fn scale_staff(&mut self, staff_y: i32, mag: i32) {
        if mag == 1024 {
            return;
        }
        let scaled = |value: i32| value * mag / 1024;
        let at = |value: i32| staff_y + scaled(value - staff_y);
        match &mut self.geometry {
            Geometry::Glyph { y, mag: m, .. } | Geometry::Path { y, mag: m, .. } => {
                *y = at(*y);
                *m = scaled(*m);
            }
            Geometry::Text { y, size, .. } => {
                *y = at(*y);
                *size = scaled(*size);
            }
            Geometry::Rect { y, w, h, .. } => {
                *y = at(*y);
                // A system's staff lines and bar lines span the shared
                // horizontal grid; their width is not a staff-size quantity.
                if self.kind != ItemKind::StaffLine
                    && self.kind != ItemKind::BarLine
                    && self.kind != ItemKind::Bracket
                {
                    *w = scaled(*w);
                }
                *h = scaled(*h);
            }
            Geometry::Beam { y1, y2, width, .. } => {
                *y1 = at(*y1);
                *y2 = at(*y2);
                *width = scaled(*width);
            }
            // Braces are stretched to the bracket height by `decor`, in
            // absolute staff spaces, and are not part of a staff's ink.
            Geometry::Scaled { .. } => {}
        }
    }

    /// Position of the item's origin in fixed-point staff spaces.
    pub const fn origin(&self) -> (i32, i32) {
        match self.geometry {
            Geometry::Glyph { x, y, .. }
            | Geometry::Path { x, y, .. }
            | Geometry::Text { x, y, .. }
            | Geometry::Scaled { x, y, .. } => (x, y),
            Geometry::Rect { x, y, .. } | Geometry::Beam { x1: x, y1: y, .. } => (x, y),
        }
    }

    /// Axis-aligned bounding box in fixed-point staff spaces.
    ///
    /// SMuFL bounding boxes use up-positive y; the display list uses
    /// down-positive y (`tdraw.cpp:2602-2614`), so the glyph's top edge is
    /// `y - bbox[3]`.
    pub fn bbox(&self, list: &DisplayList) -> Option<(i32, i32, i32, i32)> {
        match self.geometry {
            Geometry::Rect { x, y, w, h } => Some((x, y, x + w, y + h)),
            Geometry::Beam {
                x1,
                y1,
                x2,
                y2,
                width,
            } => Some((x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2) + width)),
            Geometry::Glyph { x, y, mag, .. } | Geometry::Path { x, y, mag, .. } => {
                let g = list.glyph_name(self).and_then(scoretk_smufl::glyph)?;
                let b = g.bbox;
                let scaled = |units: i16| units_to_fixed(units) * mag / 1024;
                Some((
                    x + scaled(b[0]),
                    y - scaled(b[3]),
                    x + scaled(b[2]),
                    y - scaled(b[1]),
                ))
            }
            Geometry::Scaled {
                glyph,
                x,
                y,
                sx,
                sy,
            } => {
                let g = GLYPHS.get(usize::from(glyph))?;
                let b = g.bbox;
                let left = x + i32::from(b[0]) * sx / 1024;
                let right = x + i32::from(b[2]) * sx / 1024;
                // SMuFL y is up-positive: the translated outline's lowest
                // point is `y - b[1] * sy`.
                Some((
                    left,
                    y - i32::from(b[3]) * sy / 1024,
                    right,
                    y - i32::from(b[1]) * sy / 1024,
                ))
            }
            Geometry::Text {
                codepoint,
                x,
                y,
                size,
                bold,
            } => {
                let g = scoretk_smufl::text_glyph(codepoint, bold)?;
                let scale = |units: i16| i32::from(units) * size / i32::from(scoretk_smufl::UPEM);
                Some((
                    x + scale(g.bbox[0]),
                    y - scale(g.bbox[3]),
                    x + scale(g.bbox[2]),
                    y - scale(g.bbox[1]),
                ))
            }
        }
    }
}

/// The display list of one page; allocated in the caller's arena by the host
/// (the page cache of plan §4b is not part of this crate).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayList {
    pub page: usize,
    pub items: Vec<DisplayItem>,
    /// Compact table of textual source ranges, interned by `set_note` and
    /// de-duplicated: only GABC notes carry one. The string identifier is
    /// derived from it at serialisation time.
    pub sources: Vec<NoteSourceRange>,
    /// Placement inherited by the next pushed items.
    context: SourceRef,
    /// Note identity inherited by the next pushed note-level items.
    current_note: Option<NoteIdentity>,
}

impl DisplayList {
    pub fn new(page: usize) -> Self {
        Self {
            page,
            items: Vec::new(),
            sources: Vec::new(),
            context: SourceRef::none(),
            current_note: None,
        }
    }

    /// Stamps an item with the current placement and note identity. All
    /// geometry stays byte-identical: only the two packed words change.
    pub fn push(&mut self, mut item: DisplayItem) {
        let mut source = self.context;
        if let Some(note) = &self.current_note {
            source.set_note(note, &mut self.sources);
        }
        item.source = source;
        self.items.push(item);
    }

    /// Items with no measure (title, system decorations).
    pub fn set_system(&mut self, system: usize) {
        self.context = SourceRef::system(system);
        self.current_note = None;
    }

    /// Items of one staff in the current system.
    pub fn set_staff(&mut self, system: usize, staff: usize, part: usize) {
        self.context = SourceRef::staff_placement(system, staff, part);
        self.current_note = None;
    }

    /// Items of one displayed measure; `measure` is the 0-based ordinal of
    /// `Score::measures`, i.e. `timeline.measures[].ordinal`.
    pub fn set_measure(&mut self, measure: usize, part: usize) {
        self.context = self.context.with_measure(measure, part);
    }

    /// Note identity of the next note-level items; `None` clears it.
    pub fn set_note(&mut self, note: Option<NoteIdentity>) {
        self.current_note = note;
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

    /// Applies a staff magnitude to the items emitted for one staff. The
    /// caller passes the half-open index range of that staff's items, so
    /// system decorations and other staves are untouched.
    pub fn scale_staff_range(&mut self, start: usize, end: usize, staff_y: i32, mag: i32) {
        if mag == 1024 {
            return;
        }
        for item in &mut self.items[start..end] {
            item.scale_staff(staff_y, mag);
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
            Geometry::Rect { .. }
            | Geometry::Beam { .. }
            | Geometry::Text { .. }
            | Geometry::Scaled { .. } => None,
        }
    }
}
