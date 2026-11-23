// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// ABC 2.1 §4.2 (spec lines 853-855) defines ^ as a single sharp; the v2.1 text
// does not cover the `^3^` fractional prefix. abc2xml.py's grammar
// (`alteration = digits ['/' digits]`, lines 368-369) and `decTupAcc`
// (lines 1241-1253) ignore the unsupported fraction and keep ONE sharp.
// HeadlightJig's `^3^FGA` was read as F double sharp (G) instead of F sharp.

use scoretk_core::model::*;

fn pitch(source: &str) -> u8 {
    let score = scoretk_abc_import::read_abc(source).expect("lecture ABC");
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(cr) = item {
                    if let ChordRestKind::Chord(chord) = &cr.kind {
                        if let Some(id) = chord.notes.first() {
                            return score.note(*id).pitch;
                        }
                    }
                }
            }
        }
    }
    panic!("aucune note");
}

#[test]
fn prefixe_microtonal_ne_double_pas_lalteration() {
    let header = "X:1\nM:4/4\nL:1/4\nK:C\n";
    let plain = pitch(&format!("{header}^F |]"));
    let prefixed = pitch(&format!("{header}^3^F |]"));
    assert_eq!(prefixed, plain, "^3^F doit valoir ^F (un seul dièse)");
    assert_eq!(prefixed, 66);
}
