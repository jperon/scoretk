// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A header `Q:` and an inline `[Q:]` must both emit their text.
//! `abc2xml.py::doTempo` (lines 1884-1906) writes `<words>` for each Q:
//! field. The reader kept only the last one in a single `tempo` slot.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn q_en_tete_et_en_ligne() {
    let source =
        "X:1\nM:4/4\nL:1/4\nQ:\"Easy Swing\" 1/4=140\nK:C\nC D| [Q:\"left\" 1/4=170] E F |\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains(">Easy Swing</words>"),
        "header Q: lost:\n{xml}"
    );
    assert!(xml.contains(">left</words>"), "inline Q: lost:\n{xml}");
}
