// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Quoted annotations between barlines (`"Segno"[|]"_D.S."[|]|]`) must be
//! written as `<words>`. ABC 2.1 §4.19; `abc2xml.py:2022` (`convert`) emits
//! them in the measure they appear. The reader's `finish_bar` returned
//! early for an empty measure, dropping its pending annotations.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn annotation_entre_barres() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D| \"Segno\"[|]\"_D.S.\"[|]|\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains(">Segno</words>"),
        "Segno annotation lost:\n{xml}"
    );
    assert!(
        xml.contains(">D.S.</words>"),
        "D.S. annotation lost:\n{xml}"
    );
}
