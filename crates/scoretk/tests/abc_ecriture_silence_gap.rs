// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Un silence « gap » ne compte pas dans la mesure : l'écrivain MusicXML
//! ne l'écrit pas (`exportmusicxml.rs::write_measure_tracks`). L'écrivain ABC
//! l'écrivait comme un `z` ordinaire, allongeant la mesure d'un croche et
//! décalant toutes les attaques suivantes à la relecture. ABC 2.1 § 4.5.

#![cfg(feature = "abc-export")]

use scoretk_core::model::{
    ChordRest, Clef, ClefType, Duration, Fraction, Measure, Part, Score, Segment, SegmentItem,
    SegmentType, StaffId, Track,
};

#[test]
fn silence_gap_pas_ecrit() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(StaffId::from_index(staff.index()), 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(12, 8));
    measure.len = Fraction::new(12, 8);
    let mut clef = Segment::new(SegmentType::HeaderClef, Fraction::ZERO, score.staves.len());
    clef.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G)));
    measure.add_segment(clef);

    let mut gap = ChordRest::rest(Duration::eighth());
    if let scoretk_core::model::ChordRestKind::Rest(rest) = &mut gap.kind {
        rest.gap = true;
    }
    let mut segment = Segment::new(
        SegmentType::ChordRest,
        Fraction::new(12, 8),
        score.staves.len(),
    );
    segment.set_element(track, SegmentItem::ChordRest(gap));
    measure.add_segment(segment);
    score.add_measure(measure);

    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    assert!(
        !abc.contains("z |]") && !abc.contains("z1 |]"),
        "silence gap écrit comme un silence normal :\n{abc}"
    );
}
