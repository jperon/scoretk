// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `|:|` is a combined start+end repeat barline (ABC 2.1 §4.9). The reader
//! consumed only `|:` as a start repeat and then parsed the trailing `|` as
//! a second, empty bar which reset the end barline to Normal: no `<repeat>`
//! was written at all. `abc2xml.py:428-429` parses the whole `|:|` as one
//! `bars` token and emits the repeat.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn reprise_double_en_fin_dair() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D E F | G A B c |:|\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<repeat"),
        "combined repeat barline lost:\n{xml}"
    );
}
