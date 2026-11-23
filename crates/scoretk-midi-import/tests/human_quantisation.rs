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

//! Quantification métrique d'une performance humaine courte.
//!
//! Reproduit `human_4-4.mid` des tests amont : quatre attaques décalées de 15 à
//! 30 ticks autour des temps 0, 1, 2 et 3. `human_4-4-ref.mscx` (et MuseScore
//! 4.7.4 lancé directement sur ce MIDI) note quatre noires sur ces temps.
//! `MidiBeat::findBeatLocations` refuse les fichiers de moins de 8 battements
//! (`importmidi_beat.cpp:316-330`) : c'est
//! `Quantize::applyDynamicProgramming` (`importmidi_quant.cpp:1073-1129`) qui
//! choisit alors les positions sur les niveaux métriques forts.

mod common;

use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{smf, Ev};

/// Première note : hauteur, attaque et durée en fractions de ronde.
fn first_note(score: &Score) -> (u8, Fraction, Fraction) {
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
                    let length = chord_rest
                        .ticks
                        .unwrap_or_else(|| chord_rest.duration.ticks());
                    notes.push((score.notes[note_id.index()].pitch, segment.tick, length));
                }
            }
        }
    }
    notes.sort_by_key(|(pitch, tick, _)| (*tick, *pitch));
    notes[0]
}

#[test]
fn attaque_humaine_est_ancrée_sur_le_temps_fort() {
    // Le second champ de `Ev::TimeSig` est l'exposant SMF : 2 = noire (4/4).
    let events = vec![
        (0, Ev::Tempo(500_000)),
        (0, Ev::TimeSig(4, 2)),
        (161, Ev::NoteOn(0, 42, 70)),
        (521, Ev::NoteOn(0, 45, 70)),
        (566, Ev::NoteOff(0, 42)),
        (915, Ev::NoteOff(0, 45)),
        (915, Ev::NoteOn(0, 43, 70)),
        (1354, Ev::NoteOff(0, 43)),
        (1433, Ev::NoteOn(0, 42, 70)),
        (1860, Ev::NoteOff(0, 42)),
    ];
    let score = read_midi(&smf(&[events])).expect("lecture de la performance humaine");
    assert_eq!(
        first_note(&score),
        (42, Fraction::ZERO, Fraction::new(1, 4)),
        "la première attaque doit être ancrée sur le temps fort, comme human_4-4-ref.mscx"
    );
}
