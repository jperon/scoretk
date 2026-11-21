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

/// Transposition retenue pour une piste de hauteurs `low..=high`.
fn transpose_range(program: u8, low: u8, high: u8) -> Option<(i32, i32)> {
    let mut events = vec![(0, Ev::TimeSig(4, 4)), (0, Ev::Program(0, program))];
    for (index, pitch) in (low..=high).enumerate() {
        let tick = index as u32 * 480;
        events.push((tick, Ev::NoteOn(0, pitch, 90)));
        events.push((tick + 479, Ev::NoteOff(0, pitch)));
    }
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
    // Ocarina (GM 79) : MuseScore 4.7.4 garde le gabarit préféré non
    // transpositeur (les trois fichiers `GaudeteChristusEstNatus…`).
    assert_eq!(transpose(79), None);
}

#[test]
fn plage_de_la_piste_departage_les_gabarits() {
    // Flûte (GM 73) : le gabarit préféré est `flute` (plage 59-98). Une piste
    // de 58 à 70 sort de cette plage d'un demi-ton, alors qu'`alto-flute`
    // (55-91, transposition -5) la couvre : `sortInstrumentTemplates` choisit
    // donc l'alto, comme MuseScore 4.7.4 sur `Transeamus` (piste 2 : 58-70).
    assert_eq!(transpose_range(73, 58, 70), Some((-3, -5)));
    // La même remarque vaut pour `lily-ced0e264` (55-75).
    assert_eq!(transpose_range(73, 55, 75), Some((-3, -5)));
    // Dans la plage de la flûte, le gabarit préféré reste choisi (égalité
    // d'écart, tri stable : `flute` précède `alto-flute`).
    assert_eq!(transpose_range(73, 60, 90), None);
}
