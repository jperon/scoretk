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

//! Deuxième passe de collecte des accords avant quantification.

mod common;

use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{smf_with, Ev};

fn chord_notes(score: &Score) -> Vec<(Fraction, Vec<u8>)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                let pitches: Vec<u8> = chord
                    .notes
                    .iter()
                    .map(|id| score.notes[id.index()].pitch)
                    .collect();
                result.push((segment.tick, pitches));
            }
        }
    }
    result.sort_by_key(|(tick, _)| *tick);
    result
}

/// SixNotes échelonnées d'un tick forment un accord (`chord_collect.mid`).
///
/// `quantizeAllTracks` appelle `collectChords(mtrack, {2,1}, {4,3})` juste
/// avant `quantizeChords` (`importmidi.cpp:274`) : la tolérance passe de 1/2 à
/// 4/3 de la durée minimale, ce qui regroupe les six attaques. Sans cette
/// passe, le portage n'en garde que des paires.
#[test]
fn collecte_des_accords_avant_quantification() {
    let mut events = vec![(0, Ev::TimeSig(4, 4)), (0, Ev::Tempo(500_000))];
    // Deux accords de six notes, échelonnées d'un tick (division = 120).
    for start in [5280, 5340] {
        for (i, pitch) in [43, 47, 50, 55, 62, 67].iter().enumerate() {
            events.push((start + i as u32, Ev::NoteOn(0, *pitch, 80)));
        }
        for pitch in [43, 47, 50, 55, 62, 67] {
            events.push((start + 60, Ev::NoteOff(0, pitch)));
        }
    }
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(120)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let chords = chord_notes(&score);
    let with_notes: Vec<_> = chords.iter().filter(|(_, p)| !p.is_empty()).collect();
    assert!(
        with_notes.len() >= 2,
        "deux accords attendus, obtenus {with_notes:?}"
    );
    assert!(
        with_notes.iter().any(|(_, p)| p.len() == 6),
        "les six notes échelonnées doivent former un accord : {with_notes:?}"
    );
}
