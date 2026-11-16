// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `engraving/rw/read400/tread.cpp` de MuseScore Studio, Copyright
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

//! Nuances et soufflets du lecteur 3.x.
//!
//! Portage de `TRead::read(Dynamic*)` et de la partie de
//! `TRead::read(Hairpin*)` utile à l'export MusicXML
//! (`engraving/rw/read400/tread.cpp`, partagé avec `read302`). En 3.x le
//! soufflet est encodé par un `<subtype>` numérique : 0 crescendo dessiné,
//! 1 diminuendo dessiné, 2 crescendo en texte, 3 diminuendo en texte.

use super::xml::Node;
use crate::model::{Dynamic, Hairpin, HairpinType, Placement};

/// Lit une `<Dynamic>`.
pub(crate) fn read_dynamic(node: &Node) -> Dynamic {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let custom_text = node
        .child("text")
        .map(Node::text)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    let custom = custom_text.is_some() || subtype == "other-dynamics";

    let mut dynamic = Dynamic::new(if let Some(text) = custom_text {
        text
    } else if subtype == "other-dynamics" {
        String::new()
    } else {
        subtype
    });
    dynamic.custom = custom;
    dynamic.velocity = node
        .child("velocity")
        .and_then(Node::int_text)
        .and_then(|value| u8::try_from(value.clamp(0, 127)).ok())
        .or_else(|| default_velocity(&dynamic.text));
    dynamic.placement = match node.child("placement").map(Node::trimmed_text) {
        Some(text) if text == "above" => Placement::Above,
        Some(text) if text == "below" => Placement::Below,
        _ => Placement::Below,
    };
    dynamic.visible = !node
        .child("visible")
        .and_then(Node::int_text)
        .is_some_and(|value| value == 0);
    dynamic
}

/// Vélocité par défaut de `Dynamic::velocity()`, d'après `DYN_LIST`.
fn default_velocity(text: &str) -> Option<u8> {
    let velocity = match text {
        "pppppp" => 1,
        "ppppp" => 5,
        "pppp" => 10,
        "ppp" => 16,
        "pp" => 33,
        "p" => 49,
        "mp" => 64,
        "mf" => 80,
        "f" => 96,
        "ff" => 112,
        "fff" => 126,
        "ffff" | "fffff" | "ffffff" => 127,
        "fp" => 96,
        "pf" => 49,
        "sf" | "sfz" | "sff" | "sffz" | "sfff" | "sfffz" | "sfp" | "sfpp" | "rfz" | "rf" | "fz" => {
            112
        }
        "m" => 96,
        "r" | "s" | "z" => 112,
        "n" => 49,
        _ => return None,
    };
    Some(velocity)
}

/// Lit le `<HairPin>` d'un `<Spanner type="HairPin">`.
pub(crate) fn read_hairpin(node: &Node) -> (HairpinType, Hairpin) {
    let subtype = node.child("subtype").and_then(Node::int_text).unwrap_or(0);
    let kind = if subtype == 1 || subtype == 3 {
        HairpinType::Diminuendo
    } else {
        HairpinType::Crescendo
    };
    let mut hairpin = Hairpin::new();
    hairpin.line = subtype >= 2;
    hairpin.niente = node
        .child("hairpinCircledTip")
        .and_then(Node::int_text)
        .is_some_and(|value| value != 0);
    hairpin.placement = match node.child("direction").map(Node::trimmed_text) {
        Some(text) if text == "up" => Placement::Above,
        _ => Placement::Below,
    };
    hairpin.begin_text = node.child("beginText").map(Node::text).unwrap_or_default();
    hairpin.end_text = node.child("endText").map(Node::text).unwrap_or_default();
    read_font(node, "beginFont", &mut hairpin.begin_font);
    read_font(node, "endFont", &mut hairpin.end_font);
    (kind, hairpin)
}

/// Complète une police depuis `<beginFontFace>`, `<beginFontSize>`, etc.
fn read_font(node: &Node, prefix: &str, font: &mut crate::model::LineFont) {
    if let Some(face) = node.child(&format!("{prefix}Face")) {
        font.family = face.trimmed_text();
    }
    if let Some(size) = node
        .child(&format!("{prefix}Size"))
        .and_then(|node| node.trimmed_text().parse::<f64>().ok())
    {
        font.size = size;
    }
    if let Some(style) = node.child(&format!("{prefix}Style")) {
        let style = style.trimmed_text();
        font.italic = style.contains("italic") || style == "1";
        font.bold = style.contains("bold");
    }
}
