// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `!/!`, `!//!`, `!///!` are single-note tremolos with 1, 2, 3 strokes.
//! ABC 2.1 §4.14; `abc2xml.py:1688` sets `s.trem = -len(d)` and
//! `abc2xml.py:1464-1469` writes `<tremolo type="single">n</tremolo>`.
//! The reader previously warned "unknown decoration" and dropped them.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn tremolo_simple() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n!/!C !//!D !///!E F |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<tremolo type=\"single\">1</tremolo>"),
        "one-stroke tremolo lost:\n{xml}"
    );
    assert!(
        xml.contains("<tremolo type=\"single\">2</tremolo>"),
        "two-stroke tremolo lost:\n{xml}"
    );
    assert!(
        xml.contains("<tremolo type=\"single\">3</tremolo>"),
        "three-stroke tremolo lost:\n{xml}"
    );
}
