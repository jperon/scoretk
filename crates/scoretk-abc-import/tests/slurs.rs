// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Les slurs ABC `( … )` doivent porter une `SpannerNotation::Slur`, sinon
//! l'export MusicXML les ignore (`notations.rs::slurs`).

use scoretk_core::model::{SpannerNotation, SpannerType};

#[test]
fn slur_abc_a_une_notations_de_liaison() {
    let score =
        scoretk_abc_import::read_abc("X:1\nM:4/4\nL:1/4\nK:C\n(CD) EF|\n").expect("lecture");
    let slurs: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| spanner.kind == SpannerType::Slur)
        .collect();
    assert!(!slurs.is_empty(), "aucun slur lu");
    assert!(
        slurs
            .iter()
            .all(|slur| matches!(slur.notation, Some(SpannerNotation::Slur(_)))),
        "slur sans SpannerNotation::Slur : {:?}",
        slurs.iter().map(|s| &s.notation).collect::<Vec<_>>()
    );
}
