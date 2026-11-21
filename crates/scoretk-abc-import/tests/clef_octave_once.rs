// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! An octave clef declared twice (in V: and again inline in K:) must shift
//! the pitch only once. ABC 2.1 §4.6; `abc2xml.py::doClef` lines 1736-1744.

#[test]
fn octave_clef_applied_once() {
    let source = "X:1\nM:4/4\nL:1/8\nK:F\nV:1 clef=treble-8\n[K:F clef=treble-8]C D|\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    assert_eq!(
        &score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        &[48, 50]
    );
}
