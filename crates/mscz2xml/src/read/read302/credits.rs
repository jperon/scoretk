//! `<Style>` et textes de cadre (`<VBox>`) des fichiers 3.x.
//!
//! Portage partiel de `Read302::readStyle` (`engraving/rw/read302/read302.cpp`)
//! et de `TRead::read(Text*)` pour les `Text` d'un `VBox`, que
//! `ExportMusicXml::credits` exporte en `<credit>`.

use super::xml::Node;
use crate::container::MscVersion;
use crate::model::{Credit, CreditJustify, CreditStyle, RichText, Score, StyleSettings};

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
