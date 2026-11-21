// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A quoted tempo label anywhere in `Q:` must be written as `<words>`.
//! `abc2xml.py::doTempo` (lines 1884-1887) uses
//! `re.search(r'"([^"]*)"', field)` — the label may follow the bpm
//! (`Q:1/4=80 "Slow"`) as well as precede it. ABC 2.1 §4.7. The reader only
//! looked for a leading quoted label and dropped trailing ones.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn q_label_apres_bpm() {
    let source = "X:1\nM:4/4\nL:1/4\nQ:1/4=80 \"Slow\"\nK:C\nC D E F |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains(">Slow</words>"),
        "trailing tempo label lost:\n{xml}"
    );
}
