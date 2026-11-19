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

//! Format 2 (SMF §Format 0,1,2) : les pistes sont des motifs indépendants
//! joués les uns après les autres, et non des pistes simultanées. MuseScore
//! rejette le format 2 (`midifile.cpp`) ; le portage le concatène dans le temps.

mod common;

use midly::{Format, Timing};
use midly::num::u15;
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{Ev, smf_with};

/// Deux motifs de deux noires et un motif d'une noire.
fn patterns(format: Format) -> Vec<u8> {
    let melody = |first: u8, second: u8| {
        vec![
            (0, Ev::NoteOn(0, first, 90)),
            (480, Ev::NoteOff(0, first)),
            (480, Ev::NoteOn(0, second, 90)),
            (960, Ev::NoteOff(0, second)),
        ]
    };
    smf_with(
        format,
        Timing::Metrical(u15::new(480)),
        &[
            {
                let mut track = vec![(0, Ev::Tempo(500_000)), (0, Ev::TimeSig(4, 2))];
                track.extend(melody(60, 64));
                track
            },
            melody(67, 72),
            vec![
                (0, Ev::NoteOn(0, 76, 90)),
                (480, Ev::NoteOff(0, 76)),
            ],
        ],
    )
}

fn notes(score: &Score) -> Vec<(u8, Fraction, Fraction)> {
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
                let duration = chord_rest.ticks.unwrap_or_else(|| chord_rest.duration.ticks());
                for note_id in &chord.notes {
                    result.push((score.notes[note_id.index()].pitch, segment.tick, duration));
                }
            }
        }
    }
    result.sort_by_key(|(pitch, tick, _)| (*tick, *pitch));
    result
}

#[test]
fn motifs_sequentiels_enchaines_et_format_un_inchange() {
    let score = read_midi(&patterns(Format::Sequential)).expect("lecture format 2");
    assert_eq!(score.measures.len(), 2, "mesures");
    assert_eq!(score.notes.len(), 5, "notes");
    assert_eq!(
        notes(&score),
        vec![
            (60, Fraction::ZERO, Fraction::QUARTER),
            (64, Fraction::QUARTER, Fraction::QUARTER),
            (67, Fraction::HALF, Fraction::QUARTER),
            (72, Fraction::new(3, 4), Fraction::QUARTER),
            (76, Fraction::from_integer(1), Fraction::QUARTER),
        ],
        "chaque motif commence à la fin du précédent",
    );

    // Cas non concerné : le format 1 superpose les mêmes pistes ; aucun décalage.
    let parallel = read_midi(&patterns(Format::Parallel)).expect("lecture format 1");
    assert_eq!(parallel.notes.len(), 5, "notes");
    assert!(
        notes(&parallel)
            .iter()
            .all(|(_, tick, duration)| *tick <= Fraction::QUARTER && *duration == Fraction::QUARTER),
        "format 1 : aucune piste n'est décalée dans le temps",
    );
}
