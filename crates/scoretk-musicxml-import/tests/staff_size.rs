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

//! MusicXML 4.0 `staff-details/staff-size` est un pourcentage de la taille
//! par défaut ; il alimente `Staff::user_mag_percent`, que l'engraveur utilise
//! pour `Staff::staffMag` (`staff.cpp:991-1004`).

use scoretk_musicxml_import::read_musicxml;

const TWO_STAVES: &[u8] = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <staves>2</staves>
        <staff-details number="1"><staff-lines>5</staff-lines><staff-size>100</staff-size></staff-details>
        <staff-details number="2"><staff-lines>5</staff-lines><staff-size>70</staff-size></staff-details>
        <clef number="1"><sign>G</sign><line>2</line></clef>
        <clef number="2"><sign>F</sign><line>4</line></clef>
      </attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note>
      <backup><duration>4</duration></backup>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>2</voice><type>whole</type><staff>2</staff></note>
    </measure>
  </part>
</score-partwise>"#;

#[test]
fn staff_size_parcent_est_lu_dans_user_mag_percent() {
    let score = read_musicxml(TWO_STAVES).expect("lecture");
    let staves = &score.parts[0].staves;
    assert_eq!(staves.len(), 2);
    assert_eq!(score.staff(staves[0]).user_mag_percent, 100);
    assert_eq!(score.staff(staves[1]).user_mag_percent, 70);
}

#[test]
fn staff_size_sans_staff_lines_est_aussi_lu() {
    // `read_staff_details` returns early when no `<staff-lines>` is present;
    // the size must be read before that early return.
    let document = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><attributes>
    <divisions>1</divisions>
    <staff-details><staff-size>55.5</staff-size></staff-details>
  </attributes><note><rest measure="yes"/><duration>4</duration></note></measure></part>
</score-partwise>"#;
    let score = read_musicxml(document).expect("lecture");
    let staff = score.parts[0].staves[0];
    assert_eq!(score.staff(staff).user_mag_percent, 56);
}
