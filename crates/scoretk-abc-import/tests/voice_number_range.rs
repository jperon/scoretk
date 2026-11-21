// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A score with more than four ABC voices must not panic: the reader's
//! voice-list index is not the model's 0–3 voice slot. A foreign tie on the
//! fifth voice is converted into a slur spanner; `drop_foreign_ties` used
//! the raw index to build its `Track`; `music.rs` clamps it to `% 4`.
//! ABC 2.1 §7 (voices).
//!
//! This is a rewrite of the reduced input from the deleted
//! `/tmp/abc-vcheck-run/travail/mscz/141/tk.abc`; no /tmp dependency.

#[test]
fn high_abc_voice_number_does_not_panic() {
    // Five voices; on V:5 a tie whose pitch is absent from the next note
    // is turned into a slur (`abc2xml.py::doNotations`, lines 1445-1460).
    let source = "X:1\nM:4/4\nL:1/4\nK:C\n\
                  V:1\nC D|\nV:2\nC D|\nV:3\nC D|\nV:4\nC D|\n\
                  V:5\nC- D|\n";
    let result = std::panic::catch_unwind(|| scoretk_abc_import::read_abc(source));
    assert!(result.is_ok(), "reader panicked on high voice numbers");
}
