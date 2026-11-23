// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `TRead::read(FretDiagram*)` (`engraving/rw/read460/tread.cpp:806-870`)
// de MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Grilles d'accord (`<FretDiagram>`).
//!
//! Le `.mscx` range points, marqueurs et barres sous `<fretDiagram>` ; le
//! `<Harmony>` voisin porte l'accord chiffré. Les `<string>` directs (ancien
//! format) ne sont lus par aucun lecteur du C++.

use scoretk_core::model::{
    FretBarre, FretDiagram, FretDot, FretDotType, FretMarker, FretMarkerType,
};

use super::dom::Element;

/// Lit un `<FretDiagram>`.
pub(super) fn read_fret_diagram(element: &Element) -> FretDiagram {
    let mut diagram = FretDiagram::default();
    for child in &element.children {
        match child.name.as_str() {
            "strings" => {
                if let Some(value) = child.int().and_then(|value| u8::try_from(value).ok()) {
                    diagram.strings = value;
                }
            }
            "frets" => {
                if let Some(value) = child.int().and_then(|value| u8::try_from(value).ok()) {
                    diagram.frets = value;
                }
            }
            "fretOffset" => {
                if let Some(value) = child.int().and_then(|value| u8::try_from(value).ok()) {
                    diagram.fret_offset = value;
                }
            }
            "showNut" => diagram.show_nut = child.int().unwrap_or(1) != 0,
            "fretDiagram" => {
                for item in &child.children {
                    match item.name.as_str() {
                        "string" => {
                            let Some(string) = item
                                .attr_i64("no")
                                .and_then(|value| u8::try_from(value).ok())
                            else {
                                continue;
                            };
                            for entry in &item.children {
                                match entry.name.as_str() {
                                    "dot" => {
                                        diagram.dots.push(FretDot {
                                            string,
                                            fret: entry
                                                .attr_i64("fret")
                                                .and_then(|value| u8::try_from(value).ok())
                                                .unwrap_or(0),
                                            kind: dot_type(entry.text()),
                                        });
                                    }
                                    "marker" => {
                                        diagram.markers.push(FretMarker {
                                            string,
                                            kind: marker_type(entry.text()),
                                        });
                                    }
                                    // `fingering` : lu puis ignoré par le C++.
                                    _ => {}
                                }
                            }
                        }
                        "barre" => {
                            diagram.barres.push(FretBarre {
                                start: item.attr_i64("start").unwrap_or(-1) as i8,
                                end: item.attr_i64("end").unwrap_or(-1) as i8,
                                fret: item
                                    .int()
                                    .and_then(|value| u8::try_from(value).ok())
                                    .unwrap_or(0),
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    diagram
}

/// `FretItem::nameToDotType` (`fret.cpp:1515-1524`).
fn dot_type(name: &str) -> FretDotType {
    match name {
        "cross" => FretDotType::Cross,
        "square" => FretDotType::Square,
        _ => FretDotType::Normal,
    }
}

/// `FretItem::nameToMarkerType` (`fret.cpp:1477-1486`), réduit au cercle.
fn marker_type(name: &str) -> FretMarkerType {
    match name {
        "circle" => FretMarkerType::Circle,
        _ => FretMarkerType::Other,
    }
}
