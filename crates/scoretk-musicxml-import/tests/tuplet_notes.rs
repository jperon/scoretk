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

//! Nombres bruts du `<time-modification>` : un 4:4 réduit à 1/1 porte
//! quand même quatre figures, et la base du n-olet doit rester une croche
//! (`MusicXmlTupletState::normalNotes`, `musicxmltupletstate.cpp`).

use scoretk_core::model::Fraction;
use scoretk_musicxml_import::read_musicxml;

#[test]
fn nolet_4_4_garde_sa_base_de_quart() {
    let mut xml = String::from(
        r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
"#,
    );
    for index in 0..4 {
        let notation = match index {
            0 => r#"<notations><tuplet type="start" bracket="yes"/></notations>"#,
            3 => r#"<notations><tuplet type="stop"/></notations>"#,
            _ => "",
        };
        xml.push_str(&format!(
            r#"    <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type>
      <time-modification><actual-notes>4</actual-notes><normal-notes>4</normal-notes></time-modification>
      {notation}</note>
"#
        ));
    }
    xml.push_str("  </measure></part></score-partwise>");

    let score = read_musicxml(xml.as_bytes()).expect("lecture");
    assert_eq!(score.tuplets.len(), 1, "{:?}", score.tuplets);
    assert_eq!(score.tuplets[0].ratio, Fraction::ONE);
    assert_eq!(score.tuplets[0].normal_notes, 4);
    assert_eq!(score.tuplets[0].base_len, Fraction::new(1, 4));
}
