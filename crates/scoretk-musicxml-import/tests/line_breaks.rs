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

//! `<print new-system>` / `<print new-page>` (MusicXML 4.0, §"Print").
//!
//! Portage de `MusicXmlParserPass1::print` et de
//! `createMeasuresAndVboxes` (`importmusicxmlpass1.cpp`) : la rupture
//! s'applique à la mesure *précédente*, comme un `<LayoutBreak>` de type
//! `line` ou `page` dans le `.mscx`.

use scoretk_musicxml_import::read_musicxml;

fn document(print: &str) -> Vec<u8> {
    format!(
        r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
    </measure>
    <measure number="2">
      {print}
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
    </measure>
  </part>
</score-partwise>"#
    )
    .into_bytes()
}

#[test]
fn nouveau_systeme_marque_la_mesure_precedente() {
    let score = read_musicxml(&document(r#"<print new-system="yes"/>"#)).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    assert!(score.measures[0].line_break);
    assert!(!score.measures[1].line_break);
}

#[test]
fn nouvelle_page_marque_aussi_la_mesure_precedente() {
    let score = read_musicxml(&document(r#"<print new-page="yes"/>"#)).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    assert!(score.measures[0].line_break);
    assert!(!score.measures[1].line_break);
}

#[test]
fn print_sans_rupture_ne_marque_rien() {
    let score = read_musicxml(&document("<print/>")).expect("lecture");
    assert!(!score.measures[0].line_break);
    assert!(!score.measures[1].line_break);
}
