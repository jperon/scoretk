// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/staffwrite.cpp` de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026 :
// écriture des parties, des portées, des cadres et des crédits.
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

//! Parties, portées, cadres et crédits.
//!
//! Les déclarations (`<Part>`) précèdent la musique (`<Staff id="…">`), comme
//! dans un `.mscx` de MuseScore ; les cadres de titre (`<VBox>`) et de texte
//! (`<TBox>`) sont des enfants de la première portée.

use mscz2xml::model::{
    Clef, Credit, CreditStyle, Part, Placement, RichText, ScoreMetadata, StaffBracket,
    StyleSettings,
};

use crate::text::{markup, number};
use crate::write::context::WriteContext;
use crate::write::measurewrite::write_measure;
use crate::write::twrite::write_instrument;
use crate::xmlwriter::XmlWriter;

/// Écrit le `<Style>` de la partition.
pub(crate) fn write_style(xml: &mut XmlWriter, style: &StyleSettings) {
    xml.start_element("Style", &[]);
    xml.tag("Spatium", &number(style.spatium_mm));
    xml.tag("pageWidth", &number(style.page_width_in));
    xml.tag("pageHeight", &number(style.page_height_in));
    xml.tag(
        "pageEvenLeftMargin",
        &number(style.page_even_left_margin_in),
    );
    xml.tag("pageOddLeftMargin", &number(style.page_odd_left_margin_in));
    xml.tag(
        "pageEvenRightMargin",
        &number(style.page_even_right_margin_in),
    );
    xml.tag(
        "pageOddRightMargin",
        &number(style.page_odd_right_margin_in),
    );
    xml.tag("pageEvenTopMargin", &number(style.page_even_top_margin_in));
    xml.tag("pageOddTopMargin", &number(style.page_odd_top_margin_in));
    xml.tag(
        "pageEvenBottomMargin",
        &number(style.page_even_bottom_margin_in),
    );
    xml.tag(
        "pageOddBottomMargin",
        &number(style.page_odd_bottom_margin_in),
    );
    xml.tag("pageTwosided", if style.page_two_sided { "1" } else { "0" });
    xml.tag("lyricsPlacement", placement_name(style.lyrics_placement));
    xml.tag("titleFontSize", &number(style.title_font_size));
    xml.tag("subtitleFontSize", &number(style.subtitle_font_size));
    xml.tag("composerFontSize", &number(style.composer_font_size));
    xml.tag("lyricistFontSize", &number(style.lyricist_font_size));
    xml.tag("frameFontSize", &number(style.frame_font_size));
    xml.tag(
        "instrumentExcerptFontSize",
        &number(style.instrument_excerpt_font_size),
    );
    xml.tag("footerFontSize", &number(style.footer_font_size));
    xml.tag("hairpinHeight", &number(style.hairpin_height_sp));
    xml.tag(
        "hideEmptyStaves",
        if style.hide_empty_staves { "1" } else { "0" },
    );
    xml.tag(
        "dontHideStavesInFirstSystem",
        if style.dont_hide_staves_in_first_system {
            "1"
        } else {
            "0"
        },
    );
    xml.end_element();
}

/// Nom MuseScore d'un placement.
fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Above => "above",
        Placement::Below => "below",
    }
}

/// Écrit les `<metaTag>` des métadonnées.
///
/// L'ordre suit celui de MuseScore (alphabétique par nom) ; le lecteur les
/// affecte par nom, l'ordre n'a donc pas d'effet sur le modèle.
pub(crate) fn write_meta_tags(xml: &mut XmlWriter, metadata: &ScoreMetadata) {
    let tags = [
        ("arranger", metadata.arranger.as_str()),
        ("composer", metadata.composer.as_str()),
        ("copyright", metadata.copyright.as_str()),
        ("creationDate", metadata.creation_date.as_str()),
        ("lyricist", metadata.lyricist.as_str()),
        ("movementNumber", metadata.movement_number.as_str()),
        ("movementTitle", metadata.movement_title.as_str()),
        ("mscVersion", metadata.msc_version.as_str()),
        ("platform", metadata.platform.as_str()),
        ("source", metadata.source.as_str()),
        ("subtitle", metadata.subtitle.as_str()),
        ("workNumber", metadata.work_number.as_str()),
        ("workTitle", metadata.work_title.as_str()),
    ];
    for (name, value) in tags {
        // Tous les champs sont écrits, même vides : leur présence fixe la
        // valeur lue (`platform` par défaut vaut `Linux`, un `<metaTag>`
        // présent mais vide vaut la chaîne vide).
        xml.tag_with("metaTag", &[("name", name)], value);
    }
}

/// Écrit les déclarations de parties et de portées (`<Part>`).
pub(crate) fn write_parts(xml: &mut XmlWriter, ctx: &WriteContext<'_>) {
    for (index, part) in ctx.score.parts.iter().enumerate() {
        write_part(xml, ctx, index, part);
    }
}

/// Écrit une partie et ses portées déclarées.
fn write_part(xml: &mut XmlWriter, ctx: &WriteContext<'_>, index: usize, part: &Part) {
    let id = (index + 1).to_string();
    xml.start_element("Part", &[("id", &id)]);
    if !part.name.is_empty() {
        xml.tag("trackName", &part.name);
    }
    if !part.long_name.is_empty() {
        xml.tag("name", &part.long_name);
    }
    if !part.short_name.is_empty() {
        xml.tag("shortName", &part.short_name);
    }
    write_instrument(xml, &part.instrument);
    for staff_id in &part.staves {
        let staff = ctx.score.staff(*staff_id);
        let id = (staff_id.index() + 1).to_string();
        xml.start_element("Staff", &[("id", &id)]);
        xml.start_element("StaffType", &[("group", "pitched")]);
        xml.tag("name", staff_type_name(staff.lines));
        xml.end_element();
        if staff.bar_line_span {
            xml.tag("barLineSpan", "1");
        }
        write_default_clef(xml, staff.default_clef);
        for bracket in &staff.brackets {
            write_bracket(xml, bracket);
        }
        xml.end_element();
    }
    xml.end_element();
}

/// Nom MuseScore d'un type de portée d'après son nombre de lignes.
fn staff_type_name(lines: u8) -> &'static str {
    match lines {
        1 => "perc1Line",
        2 => "perc2Line",
        3 => "perc3Line",
        4 => "perc4Line",
        _ => "stdNormal",
    }
}

/// Écrit la clef par défaut d'une portée.
fn write_default_clef(xml: &mut XmlWriter, clef: Clef) {
    if clef.concert == clef.transposed {
        xml.tag("defaultClef", crate::names::clef_type(clef.concert));
    } else {
        xml.tag("defaultConcertClef", crate::names::clef_type(clef.concert));
        xml.tag(
            "defaultTransposingClef",
            crate::names::clef_type(clef.transposed),
        );
    }
}

/// Écrit une accolade ou un crochet de portée.
fn write_bracket(xml: &mut XmlWriter, bracket: &StaffBracket) {
    let kind = bracket.kind.to_string();
    let span = bracket.span.to_string();
    let visible = if bracket.visible { "1" } else { "0" };
    xml.tag_empty(
        "bracket",
        &[("type", &kind), ("span", &span), ("visible", visible)],
    );
}

/// Écrit la musique d'une portée : cadres (première portée), puis mesures.
pub(crate) fn write_staff_music(xml: &mut XmlWriter, ctx: &WriteContext<'_>, staff_index: usize) {
    let id = (staff_index + 1).to_string();
    xml.start_element("Staff", &[("id", &id)]);
    if staff_index == 0 {
        write_frames(xml, ctx);
    }
    for measure_index in 0..ctx.score.measures.len() {
        if staff_index == 0 {
            // Cadres de texte placés au-dessus de la mesure.
            write_tboxes(xml, ctx, measure_index, Placement::Above);
        }
        write_measure(xml, ctx, measure_index, staff_index);
        if staff_index == 0 {
            // Cadres placés au-dessous de la mesure.
            write_tboxes(xml, ctx, measure_index, Placement::Below);
        }
    }
    if staff_index == 0 {
        write_page_count(xml, ctx, ctx.score.style.page_count.max(1));
    }
    xml.end_element();
}

/// Écrit les crédits (`<VBox>`) et les sauts de page qui les séparent.
fn write_frames(xml: &mut XmlWriter, ctx: &WriteContext<'_>) {
    let credits = &ctx.score.credits;
    let last_page = credits
        .iter()
        .map(|credit| credit.page.max(1))
        .max()
        .unwrap_or(1);
    for page in 1..=last_page {
        for credit in credits.iter().filter(|credit| credit.page.max(1) == page) {
            write_credit(xml, credit);
        }
        if page < last_page {
            write_page_break(xml);
        }
    }
    // Un saut de section devant la première mesure se porte sur un cadre.
    if ctx
        .score
        .measures
        .first()
        .is_some_and(|measure| measure.section_break_before)
    {
        xml.start_element("VBox", &[]);
        xml.start_element("LayoutBreak", &[]);
        xml.tag("subtype", "section");
        xml.end_element();
        xml.end_element();
    }
}

/// Écrit un crédit dans son cadre vertical.
fn write_credit(xml: &mut XmlWriter, credit: &Credit) {
    if credit.text.is_empty() {
        return;
    }
    xml.start_element("VBox", &[]);
    xml.tag("height", &number(credit.vbox_height));
    xml.start_element("Text", &[]);
    let style = credit_style_name(&credit.style);
    xml.tag("style", &style);
    xml.tag_raw("text", &markup(&credit.text));
    // `readVBox` ne lit que la première composante d'`<align>` : l'alignement
    // vertical reste celui du style.
    xml.tag("align", credit_justify_name(credit));
    xml.end_element();
    xml.end_element();
}

/// Nom MuseScore d'un alignement horizontal de crédit (`<align>`).
fn credit_justify_name(credit: &Credit) -> &'static str {
    match credit.justify {
        mscz2xml::model::CreditJustify::Left => "left",
        mscz2xml::model::CreditJustify::Center => "hcenter",
        mscz2xml::model::CreditJustify::Right => "right",
    }
}

/// Nom MuseScore d'un style de crédit.
fn credit_style_name(style: &CreditStyle) -> String {
    match style {
        CreditStyle::Title => "title".to_owned(),
        CreditStyle::Subtitle => "subtitle".to_owned(),
        CreditStyle::Composer => "composer".to_owned(),
        CreditStyle::Lyricist => "lyricist".to_owned(),
        CreditStyle::Other(name) => name.clone(),
    }
}

/// Écrit un cadre de texte (`<TBox>`) placé avant ou après une mesure.
fn write_tboxes(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    measure_index: usize,
    placement: Placement,
) {
    for text in &ctx.score.tbox_texts {
        if text.measure != measure_index || text.placement != placement {
            continue;
        }
        xml.start_element("TBox", &[]);
        xml.start_element("Text", &[]);
        if placement == Placement::Below {
            // `readTbox` reconnaît un cadre sous la mesure à son décalage
            // vertical positif, dans l'élément `<Text>`.
            xml.tag_empty("offset", &[("x", "0"), ("y", "1")]);
        }
        if let Some(size) = text.font_size {
            xml.tag("size", &number(size));
        }
        xml.tag_raw("text", &markup(&text.text));
        xml.end_element();
        xml.end_element();
    }
}

/// Écrit un cadre de saut de page (`<VBox><LayoutBreak>page`).
fn write_page_break(xml: &mut XmlWriter) {
    xml.start_element("VBox", &[]);
    xml.start_element("LayoutBreak", &[]);
    xml.tag("subtype", "page");
    xml.end_element();
    xml.end_element();
}

/// Complète le nombre de pages par des sauts vides jusqu'à `page_count`.
fn write_page_count(xml: &mut XmlWriter, ctx: &WriteContext<'_>, page_count: u32) {
    let credited_pages = ctx
        .score
        .credits
        .iter()
        .map(|credit| credit.page.max(1))
        .max()
        .unwrap_or(1);
    for _ in credited_pages..page_count {
        write_page_break(xml);
    }
}

/// Texte brut d'un [`RichText`] (utilisé par les cadres qui n'ont pas de
/// balisage).
#[allow(dead_code)]
fn plain(text: &RichText) -> &str {
    text.plain()
}
