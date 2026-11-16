//! Cadres de page en 3.x : crédits des `VBox` et sauts de page.
//!
//! Portage de `TRead::read(VBox*)` et `ExportMusicXml::credits`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`). MuseScore 3.x
//! range les textes de titre dans des `<VBox>` ; le style du texte fixe sa
//! taille et ses alignements (le fichier `.mscx` ne les répète pas).

use super::xml::Node;
use crate::model::{Credit, CreditJustify, CreditStyle, CreditValign, RichText};

/// Lit les textes d'un `<VBox>` et les rattache à une page.
pub(crate) fn read_vbox(node: &Node, page: u32) -> Vec<Credit> {
    let mut credits = Vec::new();
    for text_node in node.children_named("Text") {
        if text_node
            .child("visible")
            .and_then(Node::int_text)
            .is_some_and(|value| value == 0)
        {
            continue;
        }
        let style = text_node
            .child("style")
            .map(Node::trimmed_text)
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
        apply_v3_style(&mut credit, &style);
        // `<align>` ne fixe que l'alignement horizontal.
        if let Some(align) = text_node.child("align").map(Node::trimmed_text) {
            if let Some(horizontal) = align.split(',').next() {
                if let Some(justify) = CreditJustify::from_name(horizontal.trim()) {
                    credit.justify = justify;
                }
            }
        }
        credits.push(credit);
    }
    credits
}

/// Applique les défauts de style de MuseScore 3.x (`TextStyle`).
fn apply_v3_style(credit: &mut Credit, style: &str) {
    let (size, justify, valign) = match style {
        "title" | "Title" => (24.0, CreditJustify::Center, CreditValign::Top),
        "subtitle" | "Subtitle" => (14.0, CreditJustify::Center, CreditValign::Top),
        "composer" | "Composer" => (12.0, CreditJustify::Right, CreditValign::Bottom),
        "lyricist" | "Lyricist" | "poet" => (12.0, CreditJustify::Left, CreditValign::Bottom),
        "frame" | "Frame" => (12.0, CreditJustify::Left, CreditValign::Top),
        "instrument_excerpt" => (18.0, CreditJustify::Left, CreditValign::Top),
        _ => (10.0, CreditJustify::Center, CreditValign::Top),
    };
    credit.font_size = Some(size);
    credit.justify = justify;
    credit.valign = valign;
}

/// Vrai si le cadre ou la mesure porte un saut de page.
pub(crate) fn page_break(node: &Node) -> bool {
    node.children_named("LayoutBreak").any(|layout_break| {
        layout_break
            .child("subtype")
            .is_some_and(|subtype| subtype.trimmed_text() == "page")
    })
}
