// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `"^F"` is a placed text annotation, not a chord symbol: `abc2xml.py:415`
//! (`text_expression = Optional(oneOf('^ _ < > @')) + ...`) and line 2022
//! write it as `<words>F</words>`. ABC 2.1 §4.19. The reader stripped the
//! placement prefix then misclassified the bare letter as a chord.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn annotation_avec_prefixe_est_du_texte() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n\"^F\"C \"^P\"D E F |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains(">F</words>"),
        "placed F annotation lost:\n{xml}"
    );
    assert!(
        xml.contains(">P</words>"),
        "placed P annotation lost:\n{xml}"
    );
}
