// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read302/read302.cpp` et
// `engraving/rw/read400/tread.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! `<Style>` et textes de cadre (`<VBox>`) des fichiers 3.x.
//!
//! Portage partiel de `Read302::readStyle` (`engraving/rw/read302/read302.cpp`)
//! et de `TRead::read(Text*)` pour les `Text` d'un `VBox`, que
//! `ExportMusicXml::credits` exporte en `<credit>`.

use super::xml::Node;
use crate::container::MscVersion;
use scoretk_core::model::{Credit, CreditJustify, CreditStyle, RichText, Score, StyleSettings};

/// Lit un `<Style>` 3.x et complète les valeurs par défaut de `version`.
pub(crate) fn read_style(node: &Node, version: MscVersion) -> StyleSettings {
    let mut style = StyleSettings::for_version(version.major());
    for child in node.children() {
        style.set_value(child.name(), &child.text());
    }
    style
}

/// Vrai si l'élément porte un saut de page (`<LayoutBreak>` `page`).
pub(crate) fn page_break(node: &Node) -> bool {
    node.child("LayoutBreak").is_some_and(|layout_break| {
        layout_break
            .child("subtype")
            .is_some_and(|subtype| subtype.trimmed_text() == "page")
    })
}

/// Lit les textes d'un cadre vertical (`VBox`), exportés en `<credit>`.
pub(crate) fn read_vbox(score: &mut Score, node: &Node, page: u32) {
    let height = node
        .child("height")
        .map(|child| child.trimmed_text())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(10.0);
    for text_node in node.children_named("Text") {
        if !visible(text_node) {
            continue;
        }
        let style = text_node
            .child("style")
            .map(|child| child.trimmed_text())
            .unwrap_or_default();
        let markup = text_node
            .child("text")
            .map_or(String::new(), |child| child.inner_markup());
        let text = RichText::from_markup(&markup);
        if text.is_empty() {
            continue;
        }
        let mut credit = Credit::new(text, CreditStyle::from_name(&style));
        credit.page = page;
        credit.vbox_height = height;
        // `<align>` ne fixe que l'alignement horizontal (`Pid::POSITION`),
        // l'alignement vertical reste celui du style.
        if let Some(align) = text_node.child("align").map(|child| child.trimmed_text()) {
            if let Some(horizontal) = align.split(',').next() {
                if let Some(justify) = CreditJustify::from_name(horizontal.trim()) {
                    credit.justify = justify;
                }
            }
        }
        score.add_credit(credit);
    }
}

/// Vrai si l'élément n'est pas marqué `<visible>0</visible>`.
fn visible(node: &Node) -> bool {
    !node
        .child("visible")
        .and_then(Node::int_text)
        .is_some_and(|value| value == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{CreditJustify, Placement};

    #[test]
    fn style_et_lyrics_placement_3x() {
        let node = super::super::xml::parse(
            "<Style><lyricsPlacement>above</lyricsPlacement><pageHeight>11</pageHeight></Style>",
        )
        .expect("XML");
        let style = read_style(&node, MscVersion::from_raw(302));
        assert_eq!(style.lyrics_placement, Placement::Above);
        assert!((style.page_height_in - 11.0).abs() < 1e-9);
    }

    #[test]
    fn saut_de_page_et_credit_de_vbox() {
        let node = super::super::xml::parse(
            "<VBox><height>18</height><Text><style>composer</style>\
              <text>Compositrice</text></Text></VBox>",
        )
        .expect("XML");
        let mut score = Score::new();
        read_vbox(&mut score, &node, 2);
        assert_eq!(score.engraving.credits.len(), 1);
        assert_eq!(score.engraving.credits[0].page, 2);
        assert!((score.engraving.credits[0].vbox_height - 18.0).abs() < 1e-9);
        assert_eq!(score.engraving.credits[0].justify, CreditJustify::Right);

        let break_node = super::super::xml::parse(
            "<VBox><LayoutBreak><subtype>page</subtype></LayoutBreak></VBox>",
        )
        .expect("XML");
        assert!(page_break(&break_node));
    }

    #[test]
    fn les_paroles_suivent_lyrics_placement() {
        use scoretk_core::model::SegmentItem;

        let read_placement = |style: &str| {
            let mscx = format!(
                r#"<museScore version="3.02"><Score><Division>480</Division>{style}
                <Part><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord>
                <durationType>quarter</durationType><Lyrics><text>Do</text></Lyrics>
                <Note><pitch>60</pitch></Note></Chord></voice></Measure></Staff></Score></museScore>"#
            );
            let score = super::super::read(mscx.as_bytes(), MscVersion::from_raw(302))
                .expect("partition lue");
            let placement = score
                .measures
                .iter()
                .flat_map(|measure| &measure.segments)
                .flat_map(|segment| segment.iter_elements())
                .find_map(|(_, item)| match item {
                    SegmentItem::ChordRest(chord_rest) => {
                        chord_rest.lyrics.first().map(|lyrics| lyrics.placement)
                    }
                    _ => None,
                })
                .expect("parole");
            placement
        };

        assert_eq!(read_placement(""), Placement::Below);
        assert_eq!(
            read_placement("<Style><lyricsPlacement>above</lyricsPlacement></Style>"),
            Placement::Above
        );
    }
}
