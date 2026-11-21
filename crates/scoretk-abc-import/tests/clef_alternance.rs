// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! An alternating octave clef (`treble-8` then `treble`, repeated) carries an
//! absolute octave shift, not an increment: applying -12 every time treble-8
//! reappears accumulates -48 and makes later notes negative (Veni veni
//! Emmanuel, line 366: `B,,` with transpose -48 -> pitch -1). ABC 2.1 §4.6;
//! `abc2xml.py::doClef` lines 1736-1744.

#[test]
fn octave_clef_alternation_no_negative_pitch() {
    let source = "X:1\nM:6/4\nL:1/8\nK:G\nV:1 clef=treble\nV:2 clef=bass\n\
                  [K:G clef=treble-8]C|\n[K:G clef=treble]C|\n\
                  [K:G clef=treble-8]C|\n[K:G clef=treble]C|\n\
                  [K:G clef=treble-8]C|\n[K:G clef=treble]C|\n\
                  [K:G clef=treble-8]C|\n[K:G clef=treble]C|\n\
                  V:2\nB,,|\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    // Each alternation cancels: treble-8 -> 48, treble -> 60. The final bass
    // note keeps its written pitch (47); the old code applied -48 and failed
    // with "hauteur hors de la plage MIDI : -1".
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![48, 60, 48, 60, 48, 60, 48, 60, 47]
    );
}
