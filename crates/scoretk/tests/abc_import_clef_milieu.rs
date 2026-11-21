// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A mid-tune `K:...clef=...` at a bar start must emit its own `<clef>`.
//! ABC 2.1 §4.6 "Clefs and transposition"; `abc2xml.py:1734-1744` (`doClef`,
//! `atts.append((7, c))`) writes one `<clef>` per K:/V: clef. The reader put
//! these changes on `HeaderClef` segments, which the MusicXML exporter only
//! uses for the first measure, so later clef changes vanished.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn clef_change_en_cours_dair() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C clef=treble\nC4 |\nK:C clef=bass\nC4 |\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    let count = xml.matches("<clef").count();
    assert!(count >= 2, "mid-tune clef lost: {count} <clef> in:\n{xml}");
}
