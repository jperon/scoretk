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

//! `<defaults>` : échelle et format de page (`xsd:defaults`, `xsd:scaling`,
//! `xsd:page-layout`, `xsd:page-margins`, MusicXML 4.0, §"Document shape" de
//! `docs/agents/format-ref/musicxml.md`). Inverse de `write_defaults` de
//! `scoretk-musicxml-export/src/layout.rs` : c'est la mise en page que
//! MuseScore 4.7.4 exporte et que le lecteur ignorait jusqu'ici.

use scoretk_core::model::StyleSettings;

use crate::xml::Element;

fn number(element: &Element, name: &str) -> Option<f64> {
    element
        .child_text(name)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn margins(element: &Element, style: &mut StyleSettings, even: bool) {
    let ticks_per_inch = style.tenths_per_inch();
    if ticks_per_inch <= 0.0 {
        return;
    }
    let inches = |name| number(element, name).map(|value| value / ticks_per_inch);
    if let Some(left) = inches("left-margin") {
        if even {
            style.page_even_left_margin_in = left;
        } else {
            style.page_odd_left_margin_in = left;
        }
    }
    if let Some(right) = inches("right-margin") {
        // L'export écrit la marge droite impaire comme marge gauche paire :
        // les deux conventions sont invertibles dans les deux sens.
        if even {
            style.page_even_right_margin_in = right;
        } else {
            style.page_odd_right_margin_in = right;
        }
    }
    if let Some(top) = inches("top-margin") {
        if even {
            style.page_even_top_margin_in = top;
        } else {
            style.page_odd_top_margin_in = top;
        }
    }
    if let Some(bottom) = inches("bottom-margin") {
        if even {
            style.page_even_bottom_margin_in = bottom;
        } else {
            style.page_odd_bottom_margin_in = bottom;
        }
    }
}

/// Met à jour les champs de style que le format porte réellement.
pub(crate) fn read_defaults(root: &Element, style: &mut StyleSettings) {
    let Some(defaults) = root.child("defaults") else {
        return;
    };
    if let Some(scaling) = defaults.child("scaling") {
        if let (Some(millimeters), Some(tenths)) =
            (number(scaling, "millimeters"), number(scaling, "tenths"))
        {
            if millimeters > 0.0 && tenths > 0.0 {
                // Un spatium vaut 10 dixièmes ; MuseScore écrit 40 dixièmes
                // pour 4 spatiums (`TENTHS`).
                style.spatium_mm = millimeters * 10.0 / tenths;
            }
        }
    }
    let Some(layout) = defaults.child("page-layout") else {
        return;
    };
    let ticks_per_inch = style.tenths_per_inch();
    if ticks_per_inch <= 0.0 {
        return;
    }
    if let Some(width) = number(layout, "page-width") {
        style.page_width_in = width / ticks_per_inch;
    }
    if let Some(height) = number(layout, "page-height") {
        style.page_height_in = height / ticks_per_inch;
    }
    let mut even = false;
    let mut odd = false;
    for block in layout.children_named("page-margins") {
        match block.attribute("type") {
            Some("even") => {
                margins(block, style, true);
                even = true;
            }
            Some("odd") => {
                margins(block, style, false);
                odd = true;
            }
            _ => {
                margins(block, style, false);
                margins(block, style, true);
            }
        }
    }
    style.page_two_sided = even && odd;

    // `textAsCreditWords` (`exportmusicxml.cpp`) tire la taille de police de
    // `<credit-words font-size>` du style du titre/sous-titre/compositeur/
    // parolier. L'inverse est lisible sans ambiguïté quand `<credit-type>` est
    // présent, ce qui restaure ces quatre champs de `StyleSettings`.
    for credit in root.children_named("credit") {
        let size = credit
            .children_named("credit-words")
            .find_map(|words| words.attribute("font-size"))
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| *value > 0.0 && value.is_finite());
        let Some(size) = size else {
            continue;
        };
        match credit.child_text("credit-type").unwrap_or_default() {
            "title" => style.title_font_size = size,
            "subtitle" => style.subtitle_font_size = size,
            "composer" => style.composer_font_size = size,
            "lyricist" | "poet" => style.lyricist_font_size = size,
            _ => {}
        }
    }
}
