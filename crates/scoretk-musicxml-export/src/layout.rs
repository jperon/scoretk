// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
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

//! Mise en page exportée : `<defaults>` et positions des crédits.
//!
//! Portage de `defaults`, `writePageFormat` et `textAsCreditWords`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`). Le modèle ne
//! fait pas de mise en page ; les positions sont calculées à partir du format
//! de page et des positions par défaut des styles de texte dans un `VBox`,
//! comme le fait MuseScore pour les crédits.

use scoretk_core::model::{
    Credit, CreditJustify, CreditStyle, CreditValign, StyleSettings, TENTHS,
};

use super::exportmusicxml::Out;

/// Écrit `<defaults>` : échelle et format de page.
///
/// MuseScore écrit ensuite `<appearance>`, `<music-font>`, `<word-font>` et
/// `<lyric-font>` à partir d'une trentaine de valeurs de style qui ne sont pas
/// encore modélisées ; ces blocs restent absents (écart documenté).
pub(crate) fn write_defaults(out: &mut Out, style: &StyleSettings) {
    out.xml.start_element("defaults", &[]);

    out.xml.start_element("scaling", &[]);
    out.xml
        .tag("millimeters", &format_style_number(style.spatium_mm * 4.0));
    out.xml.tag("tenths", &TENTHS.to_string());
    out.xml.end_element();

    out.xml.start_element("page-layout", &[]);
    out.xml.tag(
        "page-height",
        &format_style_number(style.page_height_tenths()),
    );
    out.xml.tag(
        "page-width",
        &format_style_number(style.page_width_tenths()),
    );
    if style.page_two_sided {
        write_margins(
            out,
            "even",
            style.page_even_left_margin_in,
            style.page_odd_left_margin_in,
            style.page_even_top_margin_in,
            style.page_even_bottom_margin_in,
            style,
        );
        write_margins(
            out,
            "odd",
            style.page_odd_left_margin_in,
            style.page_even_left_margin_in,
            style.page_odd_top_margin_in,
            style.page_odd_bottom_margin_in,
            style,
        );
    } else {
        write_margins(
            out,
            "both",
            style.page_odd_left_margin_in,
            style.page_even_left_margin_in,
            style.page_odd_top_margin_in,
            style.page_odd_bottom_margin_in,
            style,
        );
    }
    out.xml.end_element();

    out.xml.end_element();
}

/// Écrit un bloc `<page-margins>`.
fn write_margins(
    out: &mut Out,
    kind: &str,
    left_inches: f64,
    right_inches: f64,
    top_inches: f64,
    bottom_inches: f64,
    style: &StyleSettings,
) {
    out.xml.start_element("page-margins", &[("type", kind)]);
    for (name, inches) in [
        ("left-margin", left_inches),
        ("right-margin", right_inches),
        ("top-margin", top_inches),
        ("bottom-margin", bottom_inches),
    ] {
        out.xml
            .tag(name, &format_style_number(style.tenths_from_inches(inches)));
    }
    out.xml.end_element();
}

/// Position `default-x` / `default-y` d'un crédit (`textAsCreditWords`).
///
/// `default-x` dépend de l'alignement horizontal et des marges ; `default-y`
/// part du haut de la page, moins la marge haute, puis applique le décalage du
/// style (le sous-titre est 10 mm sous le titre) et la hauteur du `VBox` pour
/// les alignements bas.
pub(crate) fn credit_position(credit: &Credit, style: &StyleSettings) -> (f64, f64) {
    let width = style.page_width_tenths();
    let height = style.page_height_tenths();
    let left = style.tenths_from_inches(style.page_odd_left_margin_in);
    let right = style.tenths_from_inches(style.page_even_left_margin_in);

    let x = match credit.justify {
        CreditJustify::Left => left,
        CreditJustify::Center => width / 2.0,
        CreditJustify::Right => width - right,
    };

    let mut y = height - style.tenths_from_inches(style.page_odd_top_margin_in);
    if matches!(credit.style, CreditStyle::Subtitle) {
        y -= style.subtitle_offset_tenths();
    }
    let height_tenths = style.vbox_height_tenths(credit.vbox_height);
    match credit.valign {
        CreditValign::Top => {}
        CreditValign::Middle | CreditValign::Baseline => y -= height_tenths / 2.0,
        CreditValign::Bottom => y -= height_tenths,
    }

    (x, y)
}

/// Position d'un crédit `rights` : bas de page, centré (`credits`).
pub(crate) fn rights_position(style: &StyleSettings) -> (f64, f64) {
    (
        style.page_width_tenths() / 2.0,
        style.tenths_from_inches(style.page_odd_bottom_margin_in),
    )
}

/// Formate un nombre comme `XmlWriter` de MuseScore : 6 chiffres
/// significatifs, zéros de fin retirés.
pub(crate) fn format_style_number(value: f64) -> String {
    if !value.is_finite() || value == 0.0 {
        return "0".to_owned();
    }
    let exponent = value.abs().log10().floor() as i32;
    let decimals = (5 - exponent).clamp(0, 10) as usize;
    let text = format!("{value:.decimals$}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_de_style_a_six_chiffres() {
        assert_eq!(format_style_number(7.05556), "7.05556");
        assert_eq!(format_style_number(1683.358939616416), "1683.36");
        assert_eq!(format_style_number(85.71428571428571), "85.7143");
        assert_eq!(format_style_number(1584.0), "1584");
        assert_eq!(format_style_number(0.0), "0");
    }

    #[test]
    fn position_des_credits() {
        let style = StyleSettings::for_version(4);
        let mut title = Credit::new(
            scoretk_core::model::RichText::new("Titre"),
            CreditStyle::Title,
        );
        let (x, y) = credit_position(&title, &style);
        assert!((x - 600.1657).abs() < 0.01);
        assert!((y - 1611.01).abs() < 0.01);

        title.style = CreditStyle::Composer;
        title.justify = CreditJustify::Right;
        title.valign = CreditValign::Bottom;
        let (x, y) = credit_position(&title, &style);
        assert!((x - 1114.62).abs() < 0.01);
        assert!((y - 1511.01).abs() < 0.01);

        title.style = CreditStyle::Subtitle;
        title.justify = CreditJustify::Center;
        title.valign = CreditValign::Top;
        let (_, y) = credit_position(&title, &style);
        assert!((y - 1553.86).abs() < 0.01);
    }
}
