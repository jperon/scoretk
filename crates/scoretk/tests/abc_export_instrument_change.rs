// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.

//! ABC 2.1 §4.6 : `transpose=` peut apparaître dans un champ en ligne. Un
//! changement d'instrument en cours de partition change la transposition
//! (`xml2abc.py::doAttr` 1278, 1319-1332) ; sans champ en ligne l'aller-retour
//! ABC la perd et transpose les notes suivantes d'un demi-ton.

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

fn instrument(name: &str, chromatic: i32) -> Instrument {
    let mut instrument = Instrument::new("wind.reed.clarinet.bflat", name, name);
    instrument.transpose = Some(Transpose::new(0, chromatic));
    instrument
}

#[test]
fn transposition_d_instrument_en_cours_de_partition() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("A Clarinet", "A Cl."));
    score.part_mut(part_id).instrument = instrument("A Clarinet", -3);
    score
        .part_mut(part_id)
        .set_instrument_at(Fraction::new(2, 1), instrument("B♭ Clarinet", -2));
    let staff = score.add_staff(part_id);
    let track = Track::new(score.staff(staff).id, 0);
    for index in 0..4i64 {
        let mut measure = Measure::new(index as usize, Fraction::new(index, 1), Fraction::ONE);
        if index == 0 {
            let mut key_seg = Segment::new(SegmentType::KeySig, Fraction::ZERO, 1);
            key_seg.set_element(track, SegmentItem::KeySig(KeySig::new(Key::E_FLAT)));
            measure.add_segment(key_seg);
        }
        let pitch = if index >= 2 { 61 } else { 60 };
        let note = score.add_note(Note::new(pitch, scoretk_core::model::tpc::E_FLAT));
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
    println!(
        "SEGS {:?}",
        score.measures[0]
            .segments
            .iter()
            .map(|s| (s.kind, s.tick, s.iter_elements().count()))
            .collect::<Vec<_>>()
    );
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    assert_eq!(
        pitches(&back),
        pitches(&score),
        "transposition perdue :\n{abc}"
    );
}
