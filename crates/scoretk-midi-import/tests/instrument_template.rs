// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Choix du gabarit d'instrument d'après le programme MIDI.

mod common;
use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_midi_import::read_midi;

fn transpose(program: u8) -> Option<(i32, i32)> {
    let events = vec![
        (0, Ev::TimeSig(4, 4)),
        (0, Ev::Program(0, program)),
        (0, Ev::NoteOn(0, 62, 90)),
        (480, Ev::NoteOff(0, 62)),
    ];
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    score
        .parts
        .first()
        .and_then(|part| part.instrument.transpose)
        .map(|tr| (tr.diatonic, tr.chromatic))
}

#[test]
fn gabarit_prefere_et_transposition() {
    // Cor en fa (GM 60) : `GM1_SOUND_DATA` → gabarit `horn`, transposé.
    assert_eq!(transpose(60), Some((-4, -7)));
    // Contrebasse (GM 43) : octave.
    assert_eq!(transpose(43), Some((-7, -12)));
    // Ocarina (GM 79) : le tri par plage de hauteur choisit ici un gabarit
    // transposé d'une quinte. C'est le comportement du port de
    // `sortInstrumentTemplates` ; MuseScore 4.7.4 garde l'ocarina préféré sur
    // `GaudeteChristusEstNatus…`, écart documenté dans `midi.md`.
    assert_eq!(transpose(79), Some((-4, -7)));
}
