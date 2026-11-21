// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `!D.C.!`, `!Fine!`, `!D.S.!` etc. are text directions (`abc2xml.py:1100`
//! `capoMap`, handled at lines 1680-1684): each writes `<words>` plus a
//! `<sound>` marker. ABC 2.1 §4.3. The reader dropped them as unknown
//! decorations.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn directive_dc_est_des_paroles() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D E F !D.C.!||\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(xml.contains(">D.C.</words>"), "D.C. direction lost:\n{xml}");
}
