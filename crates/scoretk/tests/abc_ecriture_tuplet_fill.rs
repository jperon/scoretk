// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A tuplet note's end tick must cover its scaled duration so the writer does
//! not emit a z filler that the reader counts as an extra tuplet token
//! (ABC 2.1 §4.13, `abc2xml.py::mkNote` line 1366: rests are tuplet notes).

#![cfg(feature = "abc-export")]

use scoretk_core::model::*;

fn write_tuplet() -> String {
    let mut score = Score::new();
    let p = score.add_part(Part::new("V", "V"));
    let s = score.add_staff(p);
    let t = Track::new(StaffId::from_index(s.index()), 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    m.len = Fraction::ONE;
    let mut clef = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clef.set_element(t, SegmentItem::Clef(Clef::new(ClefType::G)));
    m.add_segment(clef);
    let tup = score.add_tuplet(Tuplet::new(
        Fraction::new(2, 3),
        Fraction::new(1, 8),
        Fraction::new(1, 4),
        Fraction::new(1, 4),
    ));
    // Two consecutive tuplet notes at ticks 0 and 1/8 (the unscaled note
    // figure); their scaled ends are 3/16 and 3/8 respectively.
    for (tick, pitch) in [(Fraction::ZERO, 67u8), (Fraction::new(1, 8), 65)] {
        let id = score.add_note(Note::new(pitch, 14));
        let mut cr = ChordRest::chord(Duration::eighth());
        cr.tuplet = Some(tup);
        cr.chord_mut().unwrap().notes.push(id);
        let mut sg = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
        sg.set_element(t, SegmentItem::ChordRest(cr));
        m.add_segment(sg);
    }
    score.add_measure(m);
    scoretk_abc_export::write_abc(&score).expect("écriture")
}

#[test]
fn tuplet_round_trip_stable() {
    // A tuplet whose written figure is shorter than the group's time, so
    // the writer must account for the scale when placing fillers.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\n(2:3Gz/2Fz/2F3 |]\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
