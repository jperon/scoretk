// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A full-measure rest can carry an explicit duration (`ChordRest::ticks`)
//! longer than `Measure::len` (e.g. a multi-measure rest compressed into one
//! `<measure>` by the MusicXML importer). The ABC writer must use that
//! duration, not `Measure::len`, or the re-read shifts every later attack
//! (Anerio, `attaque_decalee`: `z8` instead of `z16`). ABC 2.1 §4.5;
//! `xml2abc.py::outVoice` writes the rest's own `abcdur`.

#![cfg(feature = "abc-export")]

use scoretk_core::model::*;

#[test]
fn rest_mesure_utilise_ticks() {
    let mut score = Score::new();
    let p = score.add_part(Part::new("V", "V"));
    let s = score.add_staff(p);
    let t = Track::new(StaffId::from_index(s.index()), 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    m.len = Fraction::ONE; // 4/4
    let mut clef = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clef.set_element(t, SegmentItem::Clef(Clef::new(ClefType::G)));
    m.add_segment(clef);
    let mut cr = ChordRest::rest(Duration::measure());
    cr.ticks = Some(Fraction::from_integer(2)); // 8 beats, not the 4 of M:4/4
    let mut sg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    sg.set_element(t, SegmentItem::ChordRest(cr));
    m.add_segment(sg);
    score.add_measure(m);
    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    assert!(
        abc.contains("z16"),
        "full-measure rest shortened to one measure: {abc}"
    );
}
