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

//! `<defaults>` : échelle et format de page (format-ref/musicxml.md,
//! §"Document shape" ; `xsd:scaling`, `xsd:page-layout`, `xsd:page-margins`).

use scoretk_core::model::Score;
use scoretk_musicxml_import::read_musicxml;

fn document(defauts: &str) -> Vec<u8> {
    format!(
        r#"<score-partwise version="4.0">
  {defauts}
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
  </measure></part>
</score-partwise>"#
    )
    .into_bytes()
}

#[test]
fn echelle_et_page_du_document_sont_lues() {
    let xml = document(
        r#"<defaults>
      <scaling><millimeters>7</millimeters><tenths>40</tenths></scaling>
      <page-layout>
        <page-height>1683.78</page-height><page-width>1190.55</page-width>
        <page-margins type="odd">
          <left-margin>72.57</left-margin><right-margin>72.57</right-margin>
          <top-margin>72.57</top-margin><bottom-margin>145.14</bottom-margin>
          </page-margins>
        </page-layout>
      </defaults>"#,
    );
    let partition = read_musicxml(&xml).expect("lecture");
    let style = &partition.engraving.style;
    assert!((style.spatium_mm - 1.75).abs() < 1e-9);
    // 25.4 pouces par mètre, 40 dixièmes pour 4 spatiums de 1,75 mm.
    let dixiemes_par_pouce = 25.4 * 40.0 / (4.0 * 1.75);
    assert!((style.page_height_in - 1683.78 / dixiemes_par_pouce).abs() < 1e-6);
    assert!((style.page_width_in - 1190.55 / dixiemes_par_pouce).abs() < 1e-6);
    assert!((style.page_odd_left_margin_in - 0.5).abs() < 1e-3);
    assert!((style.page_odd_bottom_margin_in - 1.0).abs() < 1e-3);
}

#[test]
fn aller_retour_preserve_le_format_de_page() {
    let mut partition = Score::new();
    let part = partition.add_part(scoretk_core::model::Part::new("Piano", "Pno"));
    partition.add_staff(part);
    partition.add_measure(scoretk_core::model::Measure::new(
        0,
        scoretk_core::model::Fraction::ZERO,
        scoretk_core::model::Fraction::ONE,
    ));
    partition.engraving.style.spatium_mm = 1.8;
    partition.engraving.style.page_width_in = 8.3;
    partition.engraving.style.page_height_in = 11.7;
    partition.engraving.style.page_odd_left_margin_in = 0.4;
    partition.engraving.style.page_even_bottom_margin_in = 0.8;
    let xml = scoretk_musicxml_export::export_musicxml(&partition).expect("export");
    let relue = read_musicxml(xml.as_bytes()).expect("lecture");
    let style = relue.engraving.style;
    assert!((style.spatium_mm - 1.8).abs() < 1e-3);
    assert!((style.page_width_in - 8.3).abs() < 1e-3);
    assert!((style.page_height_in - 11.7).abs() < 1e-3);
    assert!((style.page_odd_left_margin_in - 0.4).abs() < 1e-3);
    assert!((style.page_even_bottom_margin_in - 0.8).abs() < 1e-3);
}
