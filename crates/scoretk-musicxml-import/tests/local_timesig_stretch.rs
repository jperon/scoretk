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

//! Un écart de longueur entre la mesure d'une partie et la mesure globale ne
//! doit pas étirer *toutes* les mesures de la portée.
//!
//! Réduit de `Byrd_Dies.mxl` : la mesure d'ouverture porte deux silences
//! invisibles ; une mesure ultérieure, plus courte dans P1 que dans P2,
//! faisait écrire un `<backup>` et avançait la première note d'une noire.

use scoretk_core::model::Fraction;
use scoretk_musicxml_export::export_musicxml;
use scoretk_musicxml_import::read_musicxml;

const SOURCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>P1</part-name></score-part>
    <score-part id="P2"><part-name>P2</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>2</divisions>
        <time><beats>4</beats><beat-type>4</beat-type></time>
      </attributes>
      <note print-object="no"><rest/><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note print-object="no"><rest/><duration>2</duration><voice>1</voice><type>quarter</type></note>
    </measure>
    <measure number="2">
      <attributes><time><beats>4</beats><beat-type>2</beat-type></time></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes>
        <divisions>2</divisions>
        <time><beats>4</beats><beat-type>4</beat-type></time>
      </attributes>
      <note print-object="no"><rest/><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note print-object="no"><rest/><duration>2</duration><voice>1</voice><type>quarter</type></note>
    </measure>
    <measure number="2">
      <attributes><time><beats>4</beats><beat-type>2</beat-type></time></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"#;

#[test]
fn ecart_de_longueur_posterieur_netend_pas_la_premiere_mesure() {
    let score = read_musicxml(SOURCE.as_bytes()).expect("lecture");
    assert_eq!(
        score.staves[0].time_stretch,
        Fraction::ONE,
        "aucune métrique locale n'étire la portée entière"
    );
    let xml = export_musicxml(&score).expect("export");
    let first = xml
        .split("<part id=\"P1\">")
        .nth(1)
        .expect("partie P1")
        .split("</measure>")
        .next()
        .expect("première mesure");
    assert!(
        !first.contains("<backup>"),
        "les deux silences sont consécutifs : {first}"
    );
}
