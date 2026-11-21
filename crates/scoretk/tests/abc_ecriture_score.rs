// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! La ligne `%%score` d'un groupe sans accolade/crochet ne doit pas porter
//! d'espaces superflus : sinon l'aller-retour ABC l'ajoute à chaque lecture.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

use scoretk_core::model::{Part, Score};

#[test]
fn score_sans_groupe_pas_d_espaces_superflus() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno."));
    score.add_staff(part);
    score.add_staff(part);
    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    let line = abc
        .lines()
        .find(|line| line.starts_with("%%score"))
        .expect("%%score");
    assert!(!line.contains("  "), "espaces superflus : {line:?}");
    assert_eq!(line, "%%score 1 2", "ligne %%score inattendue : {line:?}");
}
