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

//! Taille implicite des crédits : `textAsCreditWords` écrit toujours
//! `font-size`, même quand le texte hérite du style (`exportmusicxml.cpp`).
//! La relecture doit donc distinguer l'héritage de la taille, sinon le champ
//! `Credit::font_size` devient explicite et l'inventaire des positions perd
//! l'objet.

use scoretk_musicxml_import::read_musicxml;

fn document(kind: &str, size: &str) -> Vec<u8> {
    format!(
        r#"<score-partwise version="4.0">
  <defaults><scaling><millimeters>7</millimeters><tenths>40</tenths></scaling><page-layout><page-width>1200</page-width><page-height>1600</page-height></page-layout></defaults>
  <credit page="1">
    {kind}
    <credit-words default-x="600" default-y="1600" justify="center" valign="top" font-size="{size}">Titre</credit-words>
    </credit>
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
    </measure></part>
</score-partwise>"#
    )
    .into_bytes()
}

#[test]
fn taille_egale_au_style_est_implicite() {
    let score =
        read_musicxml(&document("<credit-type>title</credit-type>", "24")).expect("lecture");
    assert_eq!(score.engraving.credits.len(), 1);
    assert_eq!(score.engraving.credits[0].font_size, None);
    assert_eq!(score.engraving.style.title_font_size, 24.0);
}

#[test]
fn taille_differente_du_style_reste_une_propriete_de_style() {
    let score =
        read_musicxml(&document("<credit-type>title</credit-type>", "30")).expect("lecture");
    assert_eq!(score.engraving.credits[0].font_size, None);
    assert_eq!(score.engraving.style.title_font_size, 30.0);
}

/// XSD 4.0 `xsd:credit-type` est une chaîne libre répétable : la hauteur du
/// cadre y est portée par `vbox-height=…`, sans attribut privé.
#[test]
fn hauteur_de_cadre_est_lecture() {
    let xml = String::from_utf8(document(
        "<credit-type>title</credit-type><credit-type>vbox-height=18.09770</credit-type>",
        "24",
    ))
    .expect("UTF-8");
    let score = read_musicxml(xml.as_bytes()).expect("lecture");
    assert_eq!(score.engraving.credits[0].vbox_height, 18.0977);
}

/// XSD 4.0 `xsd:formatted-text` + `xml:space="preserve"` : le texte garde ses
/// espaces finaux et ses entités littérales, sans attribut privé.
#[test]
fn texte_de_credit_sans_rogner() {
    let xml = String::from_utf8(document("<credit-type>title</credit-type>", "24"))
        .expect("UTF-8")
        .replace(
            "font-size=\"24\">Titre</credit-words>",
            "font-size=\"24\" xml:space=\"preserve\">Titre &amp;amp; </credit-words>",
        );
    let score = read_musicxml(xml.as_bytes()).expect("lecture");
    assert_eq!(score.engraving.credits[0].text.plain(), "Titre &amp; ");
}

#[test]
fn taille_sans_type_credit_reste_explicite() {
    let score = read_musicxml(&document("", "18")).expect("lecture");
    assert_eq!(score.engraving.credits[0].font_size, Some(18.0));
}
