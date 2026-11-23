// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `writeMeasureTracks` et `segmentHarmonies`
// (`importexport/musicxml/internal/export/exportmusicxml.cpp:8498-8510` et
// `:6667-6676`) : les harmonies sont cherchées sur la piste exacte
// (`seg->findAnnotations(ElementType::HARMONY, track, track)`), et seulement
// pour une piste portant un élément.

use scoretk_core::model::{
    tpc, Annotation, ChordRest, Duration, Harmony, Measure, Note, Part, Score, Segment,
    SegmentItem, SegmentType, Track,
};
use scoretk_musicxml_export::export_musicxml;

fn accord(score: &mut Score, pitch: u8, tpc: i8) -> ChordRest {
    let mut chord_rest = ChordRest::chord(Duration::half());
    chord_rest
        .chord_mut()
        .expect("accord")
        .notes
        .push(score.add_note(Note::new(pitch, tpc)));
    chord_rest
}

fn harmonie(tpc: i8) -> Annotation {
    let mut harmony = Harmony::new("F");
    harmony.root_tpc = tpc;
    harmony.kind = "normal".to_owned();
    Annotation::Harmony(harmony)
}

#[test]
fn harmonie_d_une_voix_sans_note_n_est_pas_exportee() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Piano", "Pno"));
    let staff_id = score.add_staff(part_id);
    let first = Track::new(staff_id, 0);
    let second = Track::new(staff_id, 1);
    let mut measure = Measure::new(
        0,
        scoretk_core::model::Fraction::ZERO,
        scoretk_core::model::Fraction::ONE,
    );
    let mut segment = Segment::new(
        SegmentType::ChordRest,
        scoretk_core::model::Fraction::ZERO,
        score.staves.len(),
    );
    segment.set_element(
        first,
        SegmentItem::ChordRest(accord(&mut score, 60, tpc::C)),
    );
    measure.add_segment(segment);
    let mut segment = Segment::new(
        SegmentType::ChordRest,
        scoretk_core::model::Fraction::HALF,
        score.staves.len(),
    );
    segment.add_annotation(second, harmonie(tpc::F));
    measure.add_segment(segment);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert_eq!(
        xml.matches("<harmony").count(),
        0,
        "harmonie d'une voix sans note exportée :\n{xml}"
    );
}

#[test]
fn harmonie_de_la_voix_2_est_exportee_avec_sa_note() {
    let mut score = Score::new();
    let part_id = score.add_part(Part::new("Piano", "Pno"));
    let staff_id = score.add_staff(part_id);
    let first = Track::new(staff_id, 0);
    let second = Track::new(staff_id, 1);
    let mut measure = Measure::new(
        0,
        scoretk_core::model::Fraction::ZERO,
        scoretk_core::model::Fraction::ONE,
    );
    let mut segment = Segment::new(
        SegmentType::ChordRest,
        scoretk_core::model::Fraction::ZERO,
        score.staves.len(),
    );
    segment.set_element(
        first,
        SegmentItem::ChordRest(accord(&mut score, 60, tpc::C)),
    );
    measure.add_segment(segment);
    let mut segment = Segment::new(
        SegmentType::ChordRest,
        scoretk_core::model::Fraction::HALF,
        score.staves.len(),
    );
    segment.set_element(
        second,
        SegmentItem::ChordRest(accord(&mut score, 65, tpc::F)),
    );
    segment.add_annotation(second, harmonie(tpc::F));
    measure.add_segment(segment);
    score.add_measure(measure);

    let xml = export_musicxml(&score).expect("export");
    assert_eq!(
        xml.matches("<harmony print-frame=\"no\">").count(),
        1,
        "harmonie de la voix 2 manquante :\n{xml}"
    );
    assert!(xml.contains("<root-step>F</root-step>"));
}
