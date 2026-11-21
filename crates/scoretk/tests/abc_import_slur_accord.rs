// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A slur may start inside a chord and end outside it (`[E(e]c2)`).
//! ABC 2.1 §4.11: "Both ties and slurs may be used into, out of and between
//! chords". `abc2xml.py:371-377` allows `slur_beg` as a `dec_note`
//! decoration inside `chord` (line 387). The reader ignored `(`/`)` inside
//! `[...]` as stray symbols, dropping the slur.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn slur_depuis_accord() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n[E4(e4]c2) |\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<slur"),
        "slur starting inside chord lost:\n{xml}"
    );
}
