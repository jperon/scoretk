// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A tuplet note's end tick must cover its scaled duration so the writer does
//! not emit a z filler that the reader counts as an extra tuplet token
//! (ABC 2.1 §4.13, `abc2xml.py::mkNote` line 1366: rests are tuplet notes).

#![cfg(feature = "abc-export")]

use scoretk_core::model::*;

#[test]
fn tuplet_round_trip_stable() {
    // A tuplet whose written figure is shorter than the group's time, so
    // the writer must account for the scale when placing fillers.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\n(2:3Gz/2Fz/2F3 |]\n";
    let first = scoretk_abc_import::read_abc(source).expect("lecture");
    let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
    let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let abc2 = scoretk_abc_export::write_abc(&second).expect("réécriture");
    assert_eq!(abc, abc2, "aller-retour instable :\n{abc}\n---\n{abc2}");
}
