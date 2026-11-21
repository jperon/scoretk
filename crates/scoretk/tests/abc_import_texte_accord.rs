// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `"End"` is a text annotation, not a chord symbol: `abc2xml.py`'s
//! `chord_or_text` (line 416) tries `chord_sym ^ text_expression`, and
//! `chord_sym` (lines 411-415) requires a valid `chord_kind`/degree/bass
//! after the root. `looks_like_chord` accepted any alphanumeric suffix, so
//! words beginning with A-G became `<harmony>` instead of `<words>`.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn texte_avec_racine_note_est_des_paroles() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n\"End\"C \"Am\"D E F |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains(">End</words>"),
        "text annotation misread as chord:\n{xml}"
    );
    assert!(
        xml.contains("<harmony"),
        "real chord symbol Am lost:\n{xml}"
    );
}
