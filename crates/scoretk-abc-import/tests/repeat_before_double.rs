// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Une barre double `||` suivant une fin de reprise `:|` ne doit pas écraser
//! la répétition (ABC 2.1 §4.9, `abc2xml.py::mergePartMeasure`).

use scoretk_core::model::BarLineType;

#[test]
fn reprise_avant_barre_double_conservee() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D:|\n||E F:|\n||G A:|\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let ends: Vec<BarLineType> = score.measures.iter().map(|m| m.end_bar_line).collect();
    assert_eq!(
        ends.iter().filter(|b| b.is_repeat_end()).count(),
        3,
        "reprises effacées par les barres doubles : {ends:?}"
    );
}
