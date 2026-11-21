// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `!lowermordent!` is the long ABC name for `M` and maps to MusicXML
//! `<mordent>`, just like `!mordent!`. ABC 2.1 §4.14; `abc2xml.py:1095`
//! (`ornMap`: `lowermordent` → `mordent`, `uSyms`: `M` → `lowermordent`).
//! The reader only knew the short `m` form and dropped the long name.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn lowermordent_est_un_mordent() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n!lowermordent!C !mordent!D |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert_eq!(
        xml.matches("<mordent").count(),
        2,
        "lowermordent lost:\n{xml}"
    );
}
