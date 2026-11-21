// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! An unmatched `(` still writes its `<slur type="start">`:
//! `abc2xml.py:1516-1525` emits the start on the following note whether or
//! not a `)` closes it. ABC 2.1 §4.11. The reader kept the open slur in a
//! pending list and dropped it at end of tune.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn slur_non_ferme_ecrit_quand_meme() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D (E F | G A B c |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(xml.contains("<slur"), "unmatched slur start lost:\n{xml}");
}
