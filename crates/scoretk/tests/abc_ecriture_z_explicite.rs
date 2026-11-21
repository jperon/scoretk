// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `xml2abc.py` writes an explicit `z` with the full measure multiplier for
//! a full-measure rest (e.g. `z12` in 12/8), never `Z`. `Z` makes the ABC
//! re-reader derive the rest length from the current meter, which loses the
//! written length of short/overfull measures and shifts later attacks.

#![cfg(feature = "abc-export")]

use scoretk_core::model::{Fraction, Measure, Part, Score};

#[test]
fn mesure_pleine_ecrite_z_explicite() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    score.add_staff(part);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    measure.len = Fraction::ONE;
    score.add_measure(measure);
    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    assert!(
        abc.contains("z8"),
        "expected explicit z8 for a full 4/4 measure, got:\n{abc}"
    );
    assert!(!abc.contains('Z'), "Z still emitted:\n{abc}");
}
