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
        let kind = element.child_text("credit-type").unwrap_or_default();
        // `MusicXmlParserPass1::addCreditWords` (`importmusicxmlpass1.cpp:
        // 1041-1092`) applique un `TextStyleType` d'après `<credit-type>` ;
        // `font-size` sert alors au style du texte, pas à une propriété
        // `Pid::FONT_SIZE` explicite sur l'objet. Le modèle sépare ces deux
        // faits : `StyleSettings` porte la taille, `Credit::font_size` ne
        // garde qu'une taille sans type connu.
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
            let text = first.text_trimmed();
            if text.is_empty() {
                continue;
            }
            let format = TextFormat {
                font_family: first.attribute("font-family").map(str::to_owned),
                bold: first.attribute("font-weight") == Some("bold"),
                italic: first.attribute("font-style") == Some("italic"),
                ..TextFormat::default()
            };
            let mut credit = Credit::new(RichText::from_markup_with(text, &format), style.clone());
            credit.page = page;
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
            for extra in &words[1..] {
                credit.extra_words.push(RichText::new(extra.text_trimmed()));
            }
            credits.push(credit);
        }
    }
    credits
}
