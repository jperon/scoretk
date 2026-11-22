// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! When a measure carries both a HeaderClef and a Clef segment at tick 0,
//! MuseScore writes only one `<clef>`: `findAndExportClef`
//! (`exportmusicxml.cpp:7693-7705`) prefers cs1 (previous measure's Clef, or
//! first measure's HeaderClef) over cs2 (current measure's Clef), and
//! `writeElement` skips the one already written. The port emitted both — 4
//! spurious lines in `Stella_maris_instruments - La`.

#![cfg(feature = "musicxml-export")]

use scoretk_core::model::*;
use scoretk_musicxml_export::export_musicxml;

fn score_with_both_clefs() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno."));
    let staff = score.add_staff(part);
    let track = Track::new(StaffId::from_index(staff.index()), 0);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    for kind in [SegmentType::HeaderClef, SegmentType::Clef] {
        let mut segment = Segment::new(kind, Fraction::ZERO, score.staves.len());
        segment.set_element(track, SegmentItem::Clef(Clef::new(ClefType::G8Va)));
        measure.add_segment(segment);
    }
    score.add_measure(measure);
    score
}

#[test]
fn clef_unique_en_debut_de_mesure() {
    let xml = export_musicxml(&score_with_both_clefs()).expect("export");
    let count = xml.matches("<clef>").count() + xml.matches("<clef ").count();
    assert_eq!(count, 1, "duplicate <clef> at measure start: {xml}");
}
