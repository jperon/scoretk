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

//! Ancrage d'une performance humaine : `adjustChordsToBeats` ne doit tourner
//! qu'une fois, juste après `collectChords` (`importmidi.cpp:1169-1170`).
//! Un second passage alignerait des attaques déjà alignées et décalerait le
//! début de la partition (vérifié sur `human_tempo.mid` et le lot amont).

mod common;

use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{smf, Ev};

/// Première note (hauteur, date en fractions de ronde) d'une partition.
fn first_note(score: &Score) -> (u8, Fraction) {
    let mut notes = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for note_id in &chord.notes {
                    notes.push((score.notes[note_id.index()].pitch, segment.tick));
                }
            }
        }
    }
    notes.sort_by_key(|(pitch, tick)| (*tick, *pitch));
    notes[0]
}

/// Performance humaine : 30 attaques à environ 0,31 s, avec un léger bruit.
/// MuseScore 4.7.4 place la première note à 3,25 noires (13/16 de ronde).
fn human_performance() -> Vec<u8> {
    let mut events = vec![(0, Ev::Tempo(500_000)), (0, Ev::TimeSig(4, 4))];
    for index in 0..30 {
        let onset = (480.0 * 0.31 * f64::from(index)) as u32 + (index * 17) % 53;
        let pitch = 60 + (index % 5) as u8;
        events.push((onset, Ev::NoteOn(0, pitch, 90)));
        events.push((onset + 200, Ev::NoteOff(0, pitch)));
    }
    events.sort_by_key(|(tick, _)| *tick);
    smf(&[events])
}

#[test]
fn alignment_des_temps_ne_decale_pas_la_premiere_note() {
    let score = read_midi(&human_performance()).expect("lecture de la performance humaine");
    assert_eq!(
        first_note(&score),
        (60, Fraction::new(13, 16)),
        "la première note doit rester ancrée sur la grille de temps"
    );
}
