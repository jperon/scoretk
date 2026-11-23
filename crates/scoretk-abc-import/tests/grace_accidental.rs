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

#[test]
fn naturel_explicite_dun_agrangement_reste_en_vigueur() {
    // K:Eb rend E bémol ; le `=e` de l'agrément doit neutraliser E pour la
    // suite de la mesure (Haydn, Credo/Kyrie : `=e{g/2}fe/2`).
    let header = "X:1\nM:3/4\nL:1/8\nK:Eb\n";
    let source = format!("{header}_e2 {{=e/2}}f2 e2 |]");
    assert_eq!(
        pitches(&source),
        vec![75, 77, 76],
        "le =e de l'agrément doit laisser E naturel"
    );
}
