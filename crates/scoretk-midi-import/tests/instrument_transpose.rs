// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Transposition des instruments transpositeurs à l'import MIDI.

mod common;

use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_midi_import::read_midi;

#[test]
fn contrebasse_transpose_d_une_octave() {
    let events = vec![
        (0, Ev::TimeSig(4, 4)),
        (0, Ev::Program(0, 43)), // Contrabass (GM 1-based 44)
        (0, Ev::NoteOn(0, 50, 90)),
        (480, Ev::NoteOff(0, 50)),
    ];
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let transpose = score
        .parts
        .first()
        .and_then(|part| part.instrument.transpose);
    assert!(
        transpose.is_some(),
        "la contrebasse doit porter une transposition"
    );
    assert_eq!(transpose.unwrap().chromatic, -12);
}
