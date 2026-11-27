// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Compact layout description of one engraved page, derived from the display
//! list. No dependency beyond `alloc`: the JSON writer is a small formatter,
//! not a serialisation framework. All geometry is in 1/1024 staff-space
//! units, with the same origin as the SVG viewBox.
//!
//! The `id` of a note is the stable MusicXML identifier of `write_playback`,
//! so a client can join a drawn element, a `<note>` and a `timeline` event
//! without a second index.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::display::{DisplayList, ItemKind};
use crate::layout::PageSpec;
use crate::Engraver;

/// Axis-aligned bounding box in fixed staff spaces: x, y, width, height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl BBox {
    fn grow(&mut self, other: (i32, i32, i32, i32)) {
        let left = other.0.min(other.2);
        let right = other.0.max(other.2);
        let top = other.1.min(other.3);
        let bottom = other.1.max(other.3);
        if self.w == 0 && self.h == 0 && self.x == 0 && self.y == 0 {
            *self = BBox {
                x: left,
                y: top,
                w: right - left,
                h: bottom - top,
            };
        } else {
            let new_left = self.x.min(left);
            let new_top = self.y.min(top);
            let new_right = (self.x + self.w).max(right);
            let new_bottom = (self.y + self.h).max(bottom);
            self.x = new_left;
            self.y = new_top;
            self.w = new_right - new_left;
            self.h = new_bottom - new_top;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemLayout {
    /// Index on the page, starting at zero (`data-system` in the SVG).
    pub index: usize,
    pub bbox: BBox,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureLayout {
    /// 0-based index in `Score::measures`, i.e. `timeline.measures[].ordinal`.
    pub ordinal: usize,
    pub part: usize,
    pub bbox: BBox,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteLayout {
    /// Stable MusicXML identifier, also used as the SVG `id`.
    pub id: String,
    pub bbox: BBox,
    pub page: usize,
    pub system: usize,
    pub staff: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageLayout {
    pub page: usize,
    /// Width and height of the whole page, including margins, in 1/1024 sp.
    pub size: (i32, i32),
    pub systems: Vec<SystemLayout>,
    pub measures: Vec<MeasureLayout>,
    pub notes: Vec<NoteLayout>,
}

/// Whether an item is a note-level element whose source is one `<note>`.
pub fn is_note_level(kind: ItemKind) -> bool {
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

impl PageLayout {
    /// Builds the description from one already engraved page.
    pub fn from_list(list: &DisplayList, spec: &PageSpec) -> Self {
        let mut systems: BTreeMap<usize, BBox> = BTreeMap::new();
        let mut measures: BTreeMap<(usize, usize), BBox> = BTreeMap::new();
        let mut notes: BTreeMap<String, NoteLayout> = BTreeMap::new();
        for item in &list.items {
            let Some(box_) = item.bbox(list) else {
                continue;
            };
            if let Some(system) = item.source.system_index() {
                systems.entry(system).or_default().grow(box_);
            }
            if let (Some(ordinal), Some(part)) = (item.source.measure_ordinal(), item.source.part())
            {
                measures.entry((part, ordinal)).or_default().grow(box_);
            }
            if is_note_level(item.kind) {
                if let Some(id) = item.source.xml_id(&list.sources) {
                    let entry = notes.entry(id.clone()).or_insert_with(|| NoteLayout {
                        id,
                        bbox: BBox::default(),
                        page: list.page,
                        system: item.source.system_index().unwrap_or(0),
                        staff: item.source.staff().unwrap_or(0),
                    });
                    entry.bbox.grow(box_);
                }
            }
        }
        Self {
            page: list.page,
            size: (
                spec.printable_width + 2 * spec.margin_left,
                spec.page_height,
            ),
            systems: systems
                .into_iter()
                .map(|(index, bbox)| SystemLayout { index, bbox })
                .collect(),
            measures: measures
                .into_iter()
                .map(|((part, ordinal), bbox)| MeasureLayout {
                    ordinal,
                    part,
                    bbox,
                })
                .collect(),
            notes: notes.into_values().collect(),
        }
    }

    /// JSON with integer coordinates in 1/1024 staff space. Kept compact on
    /// purpose: one page is a few hundred bytes to a few kilobytes.
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(256 + self.notes.len() * 96);
        let _ = write!(
            out,
            "{{\"page\":{},\"unit\":\"1/1024 staff space\",\"size\":[{},{}],\"systems\":[",
            self.page, self.size.0, self.size.1
        );
        for (index, system) in self.systems.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"index\":{},\"bbox\":{}}}",
                system.index,
                bbox_json(system.bbox)
            );
        }
        out.push_str("],\"measures\":[");
        for (index, measure) in self.measures.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"ordinal\":{},\"part\":{},\"bbox\":{}}}",
                measure.ordinal,
                measure.part,
                bbox_json(measure.bbox)
            );
        }
        out.push_str("],\"notes\":[");
        for (index, note) in self.notes.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"id\":\"{}\",\"bbox\":{},\"page\":{},\"system\":{},\"staff\":{}}}",
                note.id,
                bbox_json(note.bbox),
                note.page,
                note.system,
                note.staff
            );
        }
        out.push_str("]}\n");
        out
    }
}

fn bbox_json(bbox: BBox) -> String {
    format!("[{},{},{},{}]", bbox.x, bbox.y, bbox.w, bbox.h)
}

/// Equivalent of Verovio's `getPageWithElement`: the 0-based page containing
/// the first drawn element with that stable identifier.
pub fn page_of_element(engraver: &Engraver, id: &str) -> Option<usize> {
    for page in 0..engraver.page_count() {
        let list = engraver.page(page);
        for item in &list.items {
            if is_note_level(item.kind) && item.source.xml_id(&list.sources).as_deref() == Some(id)
            {
                return Some(page);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn bbox_union_covers_every_item() {
        let mut box_ = BBox::default();
        box_.grow((10, 20, 30, 40));
        box_.grow((5, 15, 25, 45));
        assert_eq!(
            box_,
            BBox {
                x: 5,
                y: 15,
                w: 25,
                h: 30
            }
        );
    }

    #[test]
    fn json_keeps_the_stable_id_and_ordinal() {
        let layout = PageLayout {
            page: 1,
            size: (100, 200),
            systems: vec![SystemLayout {
                index: 0,
                bbox: BBox {
                    x: 1,
                    y: 2,
                    w: 3,
                    h: 4,
                },
            }],
            measures: vec![MeasureLayout {
                ordinal: 7,
                part: 0,
                bbox: BBox {
                    x: 1,
                    y: 2,
                    w: 3,
                    h: 4,
                },
            }],
            notes: vec![NoteLayout {
                id: "m7-s0-v0-c0-n0".into(),
                bbox: BBox {
                    x: 1,
                    y: 2,
                    w: 3,
                    h: 4,
                },
                page: 1,
                system: 0,
                staff: 0,
            }],
        };
        let json = layout.to_json();
        assert!(json.contains("\"page\":1"));
        assert!(json.contains("\"ordinal\":7"));
        assert!(json.contains("\"id\":\"m7-s0-v0-c0-n0\""));
        assert!(json.contains("\"size\":[100,200]"));
    }
}
