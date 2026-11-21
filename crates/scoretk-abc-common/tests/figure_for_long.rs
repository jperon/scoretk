// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `figure_for` must keep the largest representable figure when the length
//! cannot be written exactly (e.g. 9/8): a triple-dotted 1024th is never a
//! sensible fallback for a 9/8 rest.

#[test]
fn figure_for_keeps_largest_fallback() {
    let (duration, exact) =
        scoretk_abc_common::duration::figure_for(scoretk_core::model::Fraction::new(9, 8));
    assert!(!exact);
    assert_eq!(duration.kind, scoretk_core::model::DurationType::Whole);
}
