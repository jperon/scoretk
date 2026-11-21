// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Deux changements de clef successifs sans note intermédiaire ne doivent
//! écrire qu'un seul champ `[K:… clef=…]` : les deux se rattachent à la même
//! note et un champ dupliqué rend l'aller-retour instable.

#![cfg(feature = "abc-export")]

use scoretk_core::model::{
    ChordRest, Clef, ClefType, Duration, Fraction, Measure, Note, Part, Score, Segment,
    SegmentItem, SegmentType, StaffId, Track,
};

#[test]
fn changements_de_clef_consecutifs_pas_dupliques() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(StaffId::from_index(staff.index()), 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    measure.len = Fraction::ONE;
    let mut clef = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clef.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G)));
    measure.add_segment(clef);
    for (tick, kind) in [
        (Fraction::new(1, 4), ClefType::F),
        (Fraction::new(1, 2), ClefType::F),
    ] {
        let mut segment = Segment::new(SegmentType::Clef, tick, score.staves.len());
        segment.set_element(track, SegmentItem::Clef(Clef::new(kind)));
        measure.add_segment(segment);
    }
    let id = score.add_note(Note::new(60, 14));
    let mut chord = ChordRest::chord(Duration::quarter());
    chord.chord_mut().expect("accord").notes.push(id);
    let mut segment = Segment::new(
        SegmentType::ChordRest,
        Fraction::new(3, 4),
        score.staves.len(),
    );
    segment.set_element(track, SegmentItem::ChordRest(chord));
    measure.add_segment(segment);
    score.add_measure(measure);

    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    let count = abc.matches("clef=bass").count();
    assert_eq!(count, 1, "champ de clef dupliqué :\n{abc}");
}
