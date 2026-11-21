// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Un changement d'armure et de clef au même tick n'écrit qu'un seul champ
//! `[K:… clef=…]` : le champ de clef contient déjà l'armure. Les deux champs
//! rendent l'aller-retour instable.

#![cfg(feature = "abc-export")]

use scoretk_core::model::{
    ChordRest, Clef, ClefType, Duration, Fraction, Key, KeySig, Measure, Note, Part, Score,
    Segment, SegmentItem, SegmentType, StaffId, Track,
};

#[test]
fn armure_et_clef_meme_tick_pas_dupliquees() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(StaffId::from_index(staff.index()), 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    measure.len = Fraction::ONE;
    let mut initial = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    initial.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G)));
    measure.add_segment(initial);
    let tick = Fraction::new(1, 2);
    let mut key = Segment::new(SegmentType::KeySig, tick, score.staves.len());
    key.set_element(track, SegmentItem::KeySig(KeySig::new(Key::G)));
    measure.add_segment(key);
    let mut clef = Segment::new(SegmentType::Clef, tick, score.staves.len());
    clef.set_element(track, SegmentItem::Clef(Clef::new(ClefType::F)));
    measure.add_segment(clef);
    let id = score.add_note(Note::new(60, 14));
    let mut chord = ChordRest::chord(Duration::quarter());
    chord.chord_mut().expect("accord").notes.push(id);
    let mut seg = Segment::new(
        SegmentType::ChordRest,
        Fraction::new(3, 4),
        score.staves.len(),
    );
    seg.set_element(track, SegmentItem::ChordRest(chord));
    measure.add_segment(seg);
    score.add_measure(measure);

    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    assert!(!abc.contains("[K:G][K:"), "champs K: dupliqués :\n{abc}");
}
