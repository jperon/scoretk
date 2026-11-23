// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// ABC 2.1 §4.2 (spec lines 853-855) defines ^, = and _ on a note; §4.20
// (line 1323) places <grace notes> *before* the note's own accidental. An
// accidental inside `{…}` belongs to the grace note only: abc2midi does not
// propagate it to the following note of the measure. The reader was calling
// read_note() for grace notes, which updated the measure accidental state.

use scoretk_core::model::*;

fn pitches(source: &str) -> Vec<u8> {
    let score = scoretk_abc_import::read_abc(source).expect("lecture ABC");
    score
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .flat_map(|s| s.iter_elements())
        .filter_map(|(_, item)| match item {
            SegmentItem::ChordRest(cr) => match &cr.kind {
                ChordRestKind::Chord(ch) => ch.notes.first().map(|id| score.note(*id).pitch),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn alterne_dun_agrangement_ne_se_propage_pas() {
    let header = "X:1\nM:3/4\nL:1/8\nK:G\n";
    let with_grace = format!("{header}{{^cde}}d2 g2 c2 |]");
    let without = format!("{header}d2 g2 c2 |]");
    assert_eq!(
        pitches(&with_grace),
        pitches(&without),
        "le ^c de l'agrément ne doit pas altérer le c suivant"
    );
    assert_eq!(pitches(&with_grace), vec![74, 79, 72]);
}
