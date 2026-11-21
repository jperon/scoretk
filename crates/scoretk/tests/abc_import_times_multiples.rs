// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Every inline `[M:x/y]` field must be written as a `<time>` element, even
//! when several occur at the same tick (`y` spacers do not advance time).
//! ABC 2.1 §3.2; `abc2xml.py::mkMeasure` writes each field in order
//! (`doTime`, lines 2272-2281). The reader kept only the last one.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn plusieurs_champs_meme_mesure() {
    let source = "X:1\nL:1/4\nK:C\n[M:2/4]y[M:3/4]y[M:4/4]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    let count = xml.matches("<time ").count() + xml.matches("<time>").count();
    assert!(
        count >= 3,
        "inline M: fields lost: {count} <time> in:\n{xml}"
    );
}
