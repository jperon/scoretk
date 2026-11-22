// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.

//! A score carrying only `poet` must not lose it in any writer or reader.
//!
//! MuseScore keeps `poet` and `lyricist` as distinct metaTags and writes both
//! back (`exportmusicxml.cpp:7237-7244` for MusicXML; the mscx writer follows
//! the same tag names). LilyPond's `\header poet` is the lyricist field
//! (`lilypond-ref/ly/titling-init.ly:87`), and MuseScore's own importer falls
//! back from `lyricist` to `poet` (`importmusicxmlpass1.cpp:1143-1146`).

#![cfg(all(
    feature = "musescore-export",
    feature = "lilypond-export",
    feature = "musicxml-import"
))]

use scoretk::{read, write, Format, Options};
use scoretk_core::model::*;

fn score_with_poet() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    score.metadata.poet = "Poete".to_owned();
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    segment.set_element(
        track,
        SegmentItem::ChordRest(ChordRest::full_measure_rest()),
    );
    measure.add_segment(segment);
    score.add_measure(measure);
    score
}

#[test]
fn poet_ecrit_par_les_ecrivains_mscx_et_lilypond() {
    let score = score_with_poet();
    let options = Options::default();
    let mscx = write(&score, Format::Mscx, &options).expect("mscx");
    let mscx = String::from_utf8(mscx).expect("UTF-8");
    assert!(
        mscx.contains("<metaTag name=\"poet\">Poete</metaTag>"),
        "le metaTag poet doit etre réécrit :\n{mscx}"
    );

    let ly = write(&score, Format::Ly, &options).expect("ly");
    let ly = String::from_utf8(ly).expect("UTF-8");
    assert!(
        ly.contains("poet = \"Poete\""),
        "le champ poet doit recevoir le poete :\n{ly}"
    );
}

#[test]
fn creator_poet_lu_sans_ecraser_lyricist() {
    let xml = br#"<score-partwise version="4.0">
  <identification>
    <creator type="lyricist">Parolier</creator>
    <creator type="poet">Poete</creator>
    <creator type="translator">Traducteur</creator>
  </identification>
  <part-list>
    <score-part id="P1"><part-name>Piano</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note><rest/><duration>4</duration></note>
    </measure>
  </part>
</score-partwise>"#;
    let score = read(xml, Format::MusicXml).expect("lecture");
    assert_eq!(score.metadata.lyricist, "Parolier");
    assert_eq!(score.metadata.poet, "Poete");
    assert_eq!(score.metadata.translator, "Traducteur");
}
