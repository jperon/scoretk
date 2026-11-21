// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Une annotation de placement vide (`">"`) est conservée : `abc2xml.py`
//! (`text_expression`, lignes 402/487 et `convert` ligne 2022) écrit un
//! `<words/>` vide en dessous de la portée.

use scoretk_core::model::Annotation;

#[test]
fn annotation_vide_conservee() {
    let score =
        scoretk_abc_import::read_abc("X:1\nM:4/4\nL:1/4\nK:C\nC \">\" D|\n").expect("lecture");
    let empty = score.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.annotations.iter())
        .filter(|entry| matches!(&entry.annotation, Annotation::StaffText(text) if text.text.is_empty()))
        .count();
    assert_eq!(empty, 1, "annotation vide perdue");
}
