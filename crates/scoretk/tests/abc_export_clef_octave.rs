// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.

//! ABC 2.1 §4.6: `clef=treble-8` transpose la lecture d'un octave. Le writer
//! doit décaler les hauteurs écrites après un changement de clef en cours de
//! mesure, sinon l'aller-retour les redescend d'un octave.

use scoretk_core::model::*;

fn pitches(score: &Score) -> Vec<u8> {
    score
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .flat_map(|s| s.iter_elements())
        .filter_map(|(_, it)| match it {
            SegmentItem::ChordRest(cr) => match &cr.kind {
                ChordRestKind::Chord(ch) => ch.notes.first().map(|id| score.note(*id).pitch),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn changement_de_clef_octave_en_cours_de_mesure() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let note = score.add_note(Note::new(76, scoretk_core::model::tpc::E));
    let mut cr = ChordRest::chord(Duration::quarter());
    cr.chord_mut().unwrap().notes.push(note);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(cr));
    measure.add_segment(seg);

    let note2 = score.add_note(Note::new(76, scoretk_core::model::tpc::E));
    let mut cr2 = ChordRest::chord(Duration::quarter());
    cr2.chord_mut().unwrap().notes.push(note2);
    let mut seg2 = Segment::new(SegmentType::ChordRest, Fraction::new(1, 4), 1);
    seg2.set_element(track, SegmentItem::ChordRest(cr2));
    measure.add_segment(seg2);

    let mut clef_seg = Segment::new(SegmentType::Clef, Fraction::new(1, 4), 1);
    clef_seg.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G8Va)));
    measure.add_segment(clef_seg);
    score.add_measure(measure);

    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    println!(
        "SEGS {:?}",
        score.measures[0]
            .segments
            .iter()
            .map(|s| (s.kind, s.tick))
            .collect::<Vec<_>>()
    );
    assert_eq!(pitches(&back), pitches(&score), " octave perdue :\n{abc}");
}

#[test]
fn clef_octave_heritee_entre_mesures() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    score.staves[staff.index()].default_clef = Clef::new(ClefType::G8Va);
    let track = Track::new(score.staff(staff).id, 0);
    for index in 0..2 {
        let mut measure = Measure::new(
            index,
            Fraction::new(index as i64, 1),
            Fraction::new(index as i64 + 1, 1),
        );
        let note = score.add_note(Note::new(76, scoretk_core::model::tpc::E));
        let mut cr = ChordRest::chord(Duration::quarter());
        cr.chord_mut().unwrap().notes.push(note);
        let mut seg = Segment::new(SegmentType::ChordRest, measure.start_tick, 1);
        seg.set_element(track, SegmentItem::ChordRest(cr));
        measure.add_segment(seg);
        score.add_measure(measure);
    }
    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    assert_eq!(pitches(&back), vec![76, 76], "clef héritée perdue :\n{abc}");
}
