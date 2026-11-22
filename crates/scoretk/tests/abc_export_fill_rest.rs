// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// ABC 2.1 §4.3 (spec line 897): `x` is an invisible rest. `xml2abc.py:724`
// fills a gap in a voice with `x`, not `z`:
//   v.append(Note(nx.tijd - vtijd, 'x'))  # fill hole with invisble rest
// A visible `z` makes the re-imported model gain a printed rest that did not
// exist in the source.

use scoretk_core::model::*;

#[test]
fn silence_de_remplissage_est_invisible() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let note = score.add_note(Note::new(60, scoretk_core::model::tpc::C));
    let mut cr = ChordRest::chord(Duration::quarter());
    cr.chord_mut().unwrap().notes.push(note);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(cr));
    measure.add_segment(seg);
    score.add_measure(measure);

    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    assert!(
        abc.contains("z6") || abc.contains("x6"),
        "trou de 3 temps :\n{abc}"
    );
    assert!(
        !abc.contains("z6"),
        "le remplissage doit être invisible (x) :\n{abc}"
    );
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    let rests: Vec<bool> = back
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .flat_map(|s| s.iter_elements())
        .filter_map(|(_, it)| match it {
            SegmentItem::ChordRest(cr) => cr.rest_ref().map(|r| r.visible),
            _ => None,
        })
        .collect();
    assert_eq!(
        rests,
        vec![false],
        "le silence de remplissage doit rester invisible :\n{abc}"
    );
}
