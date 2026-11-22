// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.

//! ABC 2.1 §4.6 (spec lines 921-933): `clef=treble-8` has the same meaning in
//! an inline `[K:…]` field as in the header — the player transposes by an
//! octave. `abc2xml.py::doClef` (1736-1744) reads the parameter even when the
//! field has no tonic.

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
fn clef_octave_en_champ_en_ligne() {
    let header = "X:1\nM:4/4\nL:1/4\nK:C\n";
    let inline = format!("{header}C4 | [K:clef=treble-8]C4 |]");
    let with_field = format!("{header}C4 | [K:C clef=treble-8]C4 |]");
    assert_eq!(
        pitches(&inline),
        pitches(&with_field),
        "un champ [K:clef=…] sans tonique doit avoir le même effet que [K:C clef=…]"
    );
    assert_eq!(
        pitches(&inline),
        vec![60, 48],
        "octave attendue : {:?}",
        pitches(&inline)
    );
}
