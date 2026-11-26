// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Réductions de `createInstruments` et `setTrackInfo` (`importmidi.cpp`) :
//! une partie à trois portées pour un orgue, et les noms du gabarit retenu.

mod common;

use scoretk_midi_import::read_midi;

use common::{smf, Ev};

#[test]
fn orgue_a_trois_portees_une_seule_partie() {
    // `areNext3OrganStaff` : programme d'orgue (16), deux pistes en grand
    // staff sur le même canal, puis une piste de pédale sur le même canal.
    let manual = vec![
        (0, Ev::Program(0, 16)),
        (0, Ev::NoteOn(0, 60, 80)),
        (959, Ev::NoteOff(0, 60)),
    ];
    let left_hand = vec![(0, Ev::NoteOn(0, 48, 80)), (959, Ev::NoteOff(0, 48))];
    let pedal = vec![(0, Ev::NoteOn(0, 36, 80)), (959, Ev::NoteOff(0, 36))];
    let score = read_midi(&smf(&[manual, left_hand, pedal])).expect("lecture");

    assert_eq!(score.parts.len(), 1, "une partie pour les trois portées");
    assert_eq!(score.parts[0].staves.len(), 3, "deux claviers et une pédale");
    assert_eq!(score.staves.len(), 3);
}

#[test]
fn noms_et_abreviation_du_gabarit() {
    let piano = vec![
        (0, Ev::Program(0, 0)),
        (0, Ev::NoteOn(0, 60, 80)),
        (959, Ev::NoteOff(0, 60)),
    ];
    let score = read_midi(&smf(&[piano])).expect("lecture");
    let part = &score.parts[0];
    // `msInstrName` du gabarit « piano »,concaténé au nom de piste MIDI.
    assert_eq!(part.long_name, "Piano, Piano");
    assert_eq!(part.name, "Piano, Piano");
    assert_eq!(part.short_name, "Pno.");
    assert_eq!(part.instrument.id, "keyboard.piano");
    assert_eq!(part.instrument.track_name, "Piano");
    assert_eq!(part.instrument.long_name, "Piano, Piano");
}
