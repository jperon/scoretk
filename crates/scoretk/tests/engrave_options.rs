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

//! Façade des options de gravure : défaut octet pour octet, relayout sans
//! relecture, et taille de portée lue depuis MusicXML.

#![cfg(feature = "engrave")]

use std::time::Instant;

use scoretk::{EngraveOptions, Format};

fn moonlight() -> scoretk::Score {
    let bytes = std::fs::read("../../tests/corpus/v3/moonlight.mscx").expect("corpus");
    scoretk::read(&bytes, Format::Mscx).expect("lecture")
}

#[test]
fn default_options_keep_the_oracle_bytes() {
    let score = moonlight();
    assert_eq!(
        scoretk::write_svg(&score, &Default::default(), 0).unwrap(),
        scoretk::write_svg_with_options(&score, &EngraveOptions::default(), 0).unwrap()
    );
    assert_eq!(
        scoretk::write_layout(&score, 0).unwrap(),
        scoretk::write_layout_with_options(&score, &EngraveOptions::default(), 0).unwrap()
    );
}

#[test]
fn relayout_changes_the_width_without_re_reading_the_score() {
    let score = moonlight();
    let options = EngraveOptions {
        page_width_px: Some(800),
        page_height_px: Some(600),
        one_system_per_page: true,
        ..EngraveOptions::default()
    };
    let mut engraver = scoretk::engrave(score, &options).unwrap();
    let before = scoretk::engrave_page_svg(&engraver, 0);
    let mut narrower = options.clone();
    narrower.page_width_px = Some(420);
    let started = Instant::now();
    engraver.relayout(&narrower).expect("relayout");
    let elapsed = started.elapsed();
    let after = scoretk::engrave_page_svg(&engraver, 0);
    eprintln!(
        "relayout moonlight.mscx 800px -> 420px: {:?}, {} -> {} bytes",
        elapsed,
        before.len(),
        after.len()
    );
    assert_ne!(before, after, "the narrower page must lay out differently");
    // The handle still owns its score: a second relayout is possible.
    engraver.relayout(&options).expect("second relayout");
    assert_eq!(scoretk::engrave_page_svg(&engraver, 0), before);
}

#[test]
fn musicxml_staff_size_reaches_the_svg() {
    const DOCUMENT: &[u8] = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><attributes>
    <divisions>1</divisions><staves>2</staves>
    <staff-details number="1"><staff-lines>5</staff-lines><staff-size>100</staff-size></staff-details>
    <staff-details number="2"><staff-lines>5</staff-lines><staff-size>70</staff-size></staff-details>
  </attributes>
  <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note>
  <backup><duration>4</duration></backup>
  <note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>2</voice><type>whole</type><staff>2</staff></note>
  </measure></part>
</score-partwise>"#;
    let score = scoretk::read(DOCUMENT, Format::MusicXml).unwrap();
    assert_eq!(score.staff(score.parts[0].staves[1]).user_mag_percent, 70);
    let svg = String::from_utf8(
        scoretk::write_svg_with_options(&score, &EngraveOptions::default(), 0).unwrap(),
    )
    .unwrap();
    assert!(svg.contains("scale(0.7"), "small staff glyphs: {svg}");
}
