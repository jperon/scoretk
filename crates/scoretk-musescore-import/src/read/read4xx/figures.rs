// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read410/tread.cpp` de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Chiffrages figurés et barres de vibrato.
//!
//! Portage de `TRead::read(FiguredBass*)`, `TRead::read(FiguredBassItem*)` et
//! `TRead::read(TremoloBar*)` (`engraving/rw/read410/tread.cpp`).

use scoretk_core::model::{
    FiguredBass, FiguredBassContLine, FiguredBassItem, FiguredBassModifier, TremoloBar,
    TremoloBarPoint, TremoloBarType,
};

use super::dom::Element;

/// Lit un `<FiguredBass>` et ses `<FiguredBassItem>`.
pub(super) fn read_figured_bass(element: &Element) -> FiguredBass {
    let mut bass = FiguredBass::new();
    for child in &element.children {
        match child.name.as_str() {
            "ticks" => {
                bass.ticks = child.fraction().unwrap_or_default();
            }
            "FiguredBassItem" => {
                let mut item = FiguredBassItem::default();
                for field in &child.children {
                    match field.name.as_str() {
                        "prefix" => {
                            item.prefix = Some(FiguredBassModifier::from_mcsx(
                                field.int().unwrap_or(0) as i32,
                            ));
                        }
                        "digit" => {
                            item.digit = field
                                .int()
                                .and_then(|value| u8::try_from(value.clamp(0, 9)).ok());
                        }
                        "suffix" => {
                            item.suffix = Some(FiguredBassModifier::from_mcsx(
                                field.int().unwrap_or(0) as i32,
                            ));
                        }
                        "continuationLine" => {
                            item.cont_line =
                                FiguredBassContLine::from_mcsx(field.int().unwrap_or(0) as i32);
                        }
                        // `TRead::read(FiguredBassItem*)` : `b0..b4` sont les
                        // cinq encadrements (`read400/tread.cpp:1322-1329`).
                        "brackets" => {
                            for (slot, name) in ["b0", "b1", "b2", "b3", "b4"].iter().enumerate() {
                                item.brackets[slot] = field
                                    .attr_i64(name)
                                    .map(|value| value.clamp(0, 4) as u8)
                                    .unwrap_or(0);
                            }
                        }
                        _ => {}
                    }
                }
                bass.items.push(item);
            }
            _ => {}
        }
    }
    bass.place_above = element
        .child("placement")
        .is_some_and(|child| child.text() == "above");
    bass.parentheses = element
        .child("parentheses")
        .and_then(Element::int)
        .is_some_and(|value| value != 0);
    bass.visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    bass
}

/// Lit un `<TremoloBar>` et ses points.
pub(super) fn read_tremolo_bar(element: &Element) -> TremoloBar {
    let mut points = Vec::new();
    for point in element.children_named("point") {
        points.push(TremoloBarPoint {
            time: point.attr_i64("time").unwrap_or(0) as i32,
            pitch: point.attr_i64("pitch").unwrap_or(0) as i32,
        });
    }
    let mut bar = TremoloBar::from_points(points);
    if let Some(kind) = element.child("type").and_then(Element::int) {
        bar.kind = match kind {
            1 => TremoloBarType::Dive,
            2 => TremoloBarType::ReleaseUp,
            3 => TremoloBarType::InvertedDip,
            4 => TremoloBarType::Return,
            5 => TremoloBarType::ReleaseDown,
            6 => TremoloBarType::Custom,
            _ => bar.kind,
        };
    }
    bar
}
