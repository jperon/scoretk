// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Une annotation (`"Segno"`) lue après la dernière note d'une mesure doit
//! rester dans cette mesure, pas dériver vers la suivante ou être perdue
//! (`abc2xml.py::convert`, `x.name == 'text'`).

use scoretk_core::model::Annotation;

fn segno_count(score: &scoretk_core::model::Score, measure: usize) -> usize {
    score.measures[measure]
        .segments
        .iter()
        .flat_map(|segment| segment.annotations.iter())
        .filter(|entry| {
            matches!(
                &entry.annotation,
                Annotation::StaffText(text) if text.text.plain() == "Segno"
            )
        })
        .count()
}

#[test]
fn annotation_avant_barre_reste_dans_sa_mesure() {
    let score = scoretk_abc_import::read_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D \"Segno\"|\nE F|\n")
        .expect("lecture");
    assert_eq!(
        segno_count(&score, 0),
        1,
        "annotation dérivée vers la mesure 2"
    );
}

#[test]
fn annotation_en_fin_dair_nest_pas_perdue() {
    let score =
        scoretk_abc_import::read_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D \"Segno\"|\n").expect("lecture");
    assert_eq!(segno_count(&score, 0), 1, "annotation finale perdue");
}
