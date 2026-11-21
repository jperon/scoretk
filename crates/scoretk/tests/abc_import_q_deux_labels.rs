// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A Q: field with two quoted labels (`[Q:"left" 1/4=170"right"]`) must
//! still yield its tempo direction. `abc2xml.py:1886` finds the first
//! quoted label; the remaining quoted text must not break the bpm parse.
//! ABC 2.1 §4.7.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn q_deux_labels() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC4| [Q:\"left\" 1/4=170\"right\"] D4 |\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains(">left</words>"),
        "inline Q: label lost:\n{xml}"
    );
}
