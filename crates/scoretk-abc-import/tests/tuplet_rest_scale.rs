// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! ABC 2.1 §4.13: `(p:q:r` puts p notes into the time of q for the next r
//! notes, q defaulting to 3 in compound meter. `abc2xml.py` sets
//! `tmnum, tmden, ntup = n, into, nts` (line 2018) and scales every event
//! including rests by `tmden/tmnum` (line 1366); rests count toward `ntup`
//! (line 1306). Our reader applied the 3/2 scale to notes but not rests,
//! reading `(2:3ez/2fz/2` in 12/8 as 7/16 instead of 15/32.

#[test]
fn rest_inside_tuplet_is_scaled() {
    let source = "X:1\nM:12/8\nL:1/8\nK:Eb\n(2:3ez/2fz/2 |\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    assert_eq!(
        score.measures[0].len,
        scoretk_core::model::Fraction::new(15, 32),
        "rest inside (2:3 tuplet must be scaled by 3/2"
    );
}
