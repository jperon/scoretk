// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass1.cpp`
// (`MusicXmlParserPass1::credit`) de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Crédits (`<credit>` : titre, sous-titre, compositeur, parolier, droits).
//!
//! L'import de MuseScore passe par un cadre vertical par page ; le modèle de
//! `scoretk_core` ne garde que ce que l'export écrit : le texte, son style
//! (`<credit-type>`), ses alignements et sa taille de police.

use scoretk_core::model::{Credit, CreditJustify, CreditStyle, CreditValign, RichText, TextFormat};

use crate::xml::Element;

/// Lit les `<credit>` du document, dans l'ordre des pages.
pub(crate) fn read_credits(root: &Element) -> Vec<Credit> {
    let mut credits = Vec::new();
    for element in root.children_named("credit") {
        let page = element
            .attribute("page")
            .and_then(|page| page.parse::<u32>().ok())
            .unwrap_or(1);
        // `xsd:credit-type` est une chaîne libre et peut être répété. La
        // hauteur du cadre vertical est portée par une valeur dédiée
        // `vbox-height=…` (voir musicxml-export.md §55), sans attribut privé.
        let mut vbox_height: Option<f64> = None;
        let mut kind = "";
        for value in element.children_named("credit-type") {
            let value = value.text_trimmed();
            if let Some(height) = value.strip_prefix("vbox-height=") {
                vbox_height = height.parse::<f64>().ok();
            } else if kind.is_empty() {
                kind = value;
            }
        }
        // `MusicXmlParserPass1::addCreditWords` (`importmusicxmlpass1.cpp:
        // 1041-1092`) applique un `TextStyleType` d'après `<credit-type>`.
        let typed = !kind.is_empty() && kind != "rights";
        let style = match kind {
            "title" => CreditStyle::Title,
            "subtitle" => CreditStyle::Subtitle,
            "composer" => CreditStyle::Composer,
            "lyricist" | "poet" => CreditStyle::Lyricist,
            "rights" => continue,
            "" => CreditStyle::Other(String::new()),
            other => CreditStyle::Other(other.to_owned()),
        };
        let words: Vec<&Element> = element.children_named("credit-words").collect();
        let Some(first) = words.first() else {
            continue;
        };
        {
            // XSD 4.0 `xsd:formatted-text` : le texte est conservé tel quel
            // (les blancs et les retours à la ligne font partie du modèle).
            // Les `<credit-words>` suivants sont les lignes suivantes d'un
            // même cadre ; `xml:space="preserve"` est écrit par notre export.
            let mut text = first.text.clone();
            for extra in &words[1..] {
                text.push_str(&extra.text);
            }
            if text.trim().is_empty() {
                continue;
            }
            let format = TextFormat {
                font_family: first.attribute("font-family").map(str::to_owned),
                bold: first.attribute("font-weight") == Some("bold"),
                italic: first.attribute("font-style") == Some("italic"),
                ..TextFormat::default()
            };
            let mut credit = Credit::new(RichText::from_markup_with(&text, &format), style.clone());
            credit.page = page;
            if let Some(height) = vbox_height {
                credit.vbox_height = height;
            }
            if let Some(justify) = first
                .attribute("justify")
                .and_then(CreditJustify::from_name)
            {
                credit.justify = justify;
            }
            if let Some(valign) = first.attribute("valign").and_then(CreditValign::from_name) {
                credit.valign = valign;
            }
            if let Some(size) = first
                .attribute("font-size")
                .and_then(|size| size.parse::<f64>().ok())
            {
                if !typed {
                    credit.font_size = Some(size);
                }
            }
            credits.push(credit);
        }
    }
    credits
}
