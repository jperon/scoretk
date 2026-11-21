// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Barres de reprise `:|` suivies de `|:` (norme ABC 2.1 §4.9).

use scoretk_core::model::BarLineType;

#[test]
fn reprise_finie_puis_reouverte() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D:|\n|: E F|\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    assert_eq!(score.measures[0].end_bar_line, BarLineType::EndRepeat);
    assert_eq!(score.measures[1].start_bar_line, BarLineType::StartRepeat);
}
