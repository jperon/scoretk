// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// ABC 2.1 §4.6 (spec 921-933): `clef=` and the key tonic are independent
// parameters of the same inline K: field. xml2abc.py writes `[K:C][K:treble]`
// for copypaste_parts' bar 3; scoretk wrote `[K:E clef=treble]` because
// draw_context_changes ran before the measure-start key update and used the
// previous measure's key, turning the following F natural into F sharp.

use scoretk_core::model::*;

#[test]
fn clef_en_cours_de_mesure_utilise_la_tonalite_courante() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    for index in 0..2i64 {
        let mut measure = Measure::new(index as usize, Fraction::new(index, 1), Fraction::ONE);
        let mut key_seg = Segment::new(SegmentType::KeySig, measure.start_tick, 1);
        key_seg.set_element(
            track,
            SegmentItem::KeySig(KeySig::new(if index == 0 { Key::E } else { Key::C })),
        );
        measure.add_segment(key_seg);
        for tick in [0i64, 1, 2, 3] {
            let note = score.add_note(Note::new(60, scoretk_core::model::tpc::C));
            let mut cr = ChordRest::chord(Duration::quarter());
            cr.chord_mut().unwrap().notes.push(note);
            let mut seg = Segment::new(SegmentType::ChordRest, Fraction::new(tick, 1), 1);
            seg.set_element(track, SegmentItem::ChordRest(cr));
            measure.add_segment(seg);
        }
        if index == 1 {
            let mut clef_seg = Segment::new(SegmentType::Clef, Fraction::new(2, 1), 1);
            clef_seg.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G)));
            measure.add_segment(clef_seg);
        }
        score.add_measure(measure);
    }
    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    assert!(
        !abc.contains("[K:E clef="),
        "clef avec tonalité périmée :\n{abc}"
    );
    assert!(
        abc.contains("[K:C clef="),
        "la clef doit porter la tonalité courante :\n{abc}"
    );
}
