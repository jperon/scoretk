// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A tie on a single note of a chord must not tie every note of the chord.
//! ABC 2.1 §4.2 (`abc2xml.py:387`: `chord_note = dec_note`, and `dec_note`
//! carries `Optional(tie)` at line 383). The writer used a chord-level `-`
//! whenever any note was tied; the reader must also honour per-note ties
//! inside `[...]`.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

use scoretk_core::model::*;

#[test]
fn tie_partiel_dans_accord() {
    let mut score = Score::new();
    let p = score.add_part(Part::new("V", "V"));
    let s = score.add_staff(p);
    let t = Track::new(StaffId::from_index(s.index()), 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    m.len = Fraction::ONE;
    let mut clef = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clef.set_element(t, SegmentItem::Clef(Clef::new(ClefType::G)));
    m.add_segment(clef);

    // Two consecutive chords; only the C is tied from the first to the second.
    let mut first = ChordRest::chord(Duration::quarter());
    let c1 = score.add_note(Note::new(60, 14));
    let e1 = score.add_note(Note::new(64, 18));
    let g1 = score.add_note(Note::new(67, 15));
    first.chord_mut().unwrap().notes.extend([c1, e1, g1]);
    let mut second = ChordRest::chord(Duration::quarter());
    let c2 = score.add_note(Note::new(60, 14));
    let e2 = score.add_note(Note::new(64, 18));
    let g2 = score.add_note(Note::new(67, 15));
    second.chord_mut().unwrap().notes.extend([c2, e2, g2]);
    score.note_mut(c1).tie_forward = Some(c2);
    score.note_mut(c2).tie_back = Some(c1);

    let mut sg1 = Segment::new(SegmentType::ChordRest, Fraction::ZERO, score.staves.len());
    sg1.set_element(t, SegmentItem::ChordRest(first));
    m.add_segment(sg1);
    let mut sg2 = Segment::new(
        SegmentType::ChordRest,
        Fraction::new(1, 4),
        score.staves.len(),
    );
    sg2.set_element(t, SegmentItem::ChordRest(second));
    m.add_segment(sg2);
    score.add_measure(m);

    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    assert!(abc.contains("[C-EG]"), "per-note chord tie lost: {abc}");
    for l in abc.lines() {
        println!("ABC {l}");
    }
    let read = scoretk_abc_import::read_abc(&abc).expect("lecture");
    for n in &read.notes {
        println!(
            "NOTE pitch {} tie_f {:?} tie_b {:?}",
            n.pitch, n.tie_forward, n.tie_back
        );
    }
    let tied: Vec<u8> = read
        .notes
        .iter()
        .filter(|n| n.tie_forward.is_some())
        .map(|n| n.pitch)
        .collect();
    assert_eq!(tied, vec![60], "wrong notes tied on re-read: {tied:?}");
}
