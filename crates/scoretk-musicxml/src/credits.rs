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
//! `mscz2xml` ne garde que ce que l'export écrit : le texte, son style
//! (`<credit-type>`), ses alignements et sa taille de police.

use mscz2xml::model::{Credit, CreditJustify, CreditStyle, CreditValign, RichText, TextFormat};

use crate::xml::Element;

/// Lit les `<credit>` du document, dans l'ordre des pages.
pub(crate) fn read_credits(root: &Element) -> Vec<Credit> {
    let mut credits = Vec::new();
    for element in root.children_named("credit") {
        let page = element
            .attribute("page")
            .and_then(|page| page.parse::<u32>().ok())
            .unwrap_or(1);
        let kind = element.child_text("credit-type").unwrap_or_default();
        let style = match kind {
            "title" => CreditStyle::Title,
            "subtitle" => CreditStyle::Subtitle,
            "composer" => CreditStyle::Composer,
            "lyricist" | "poet" => CreditStyle::Lyricist,
            "rights" => CreditStyle::Other("rights".to_owned()),
            "" => CreditStyle::Other(String::new()),
            other => CreditStyle::Other(other.to_owned()),
        };
        for words in element.children_named("credit-words") {
            let text = words.text_trimmed();
            if text.is_empty() {
                continue;
            }
            let mut format = TextFormat::default();
            format.font_family = words.attribute("font-family").map(str::to_owned);
            format.bold = words.attribute("font-weight") == Some("bold");
            format.italic = words.attribute("font-style") == Some("italic");
            let mut credit = Credit::new(RichText::from_markup_with(text, &format), style.clone());
            credit.page = page;
            if let Some(justify) = words
                .attribute("justify")
                .and_then(CreditJustify::from_name)
            {
                credit.justify = justify;
            }
            if let Some(valign) = words.attribute("valign").and_then(CreditValign::from_name) {
                credit.valign = valign;
            }
            if let Some(size) = words
                .attribute("font-size")
                .and_then(|size| size.parse::<f64>().ok())
            {
                credit.font_size = Some(size);
            }
            credits.push(credit);
        }
    }
    credits
}
