// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Une liaison `-` entre deux hauteurs différentes devient un slur
//! (`abc2xml.py::doNotations`, lignes 1445-1460 ; ABC 2.1 §4.14).

use scoretk_core::model::SpannerType;

#[test]
fn liaison_entre_hauteurs_differentes_devient_slur() {
    let score = scoretk_abc_import::read_abc("X:1\nM:4/4\nL:1/4\nK:C\nC-D|\n").expect("lecture");
    let slurs = score
        .spanners
        .iter()
        .filter(|spanner| spanner.kind == SpannerType::Slur)
        .count();
    assert_eq!(slurs, 1, "slur de la liaison invalide manquant");
    assert!(
        score.notes.iter().all(|note| note.tie_forward.is_none()),
        "liaison invalide laissée sur les notes"
    );
}
