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

//! Division SMPTE (SMF §Header Chunk) : `frames/s × ticks/frame` est un temps
//! réel, pas un tick de noire. MuseScore ne consulte pas la carte des tempos
//! ici, mais convertit avec une équivalence fixe à 120 BPM ; le portage fait
//! de même, en tenant compte de la division réelle.

mod common;

use midly::{Fps, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{smf_with, Ev};

/// Notes `(hauteur, attaque, durée)` en fractions de ronde.
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
                let duration = chord_rest
                    .ticks
                    .unwrap_or_else(|| chord_rest.duration.ticks());
                for note_id in &chord.notes {
                    result.push((score.notes[note_id.index()].pitch, segment.tick, duration));
                }
            }
        }
    }
    result.sort_by_key(|(pitch, tick, _)| (*tick, *pitch));
    result
}

/// Quatre blanches SMPTE : une par seconde à 120 BPM, soit deux noires.
fn smpte(fps: Fps, subframes: u8, division: i64) -> Vec<u8> {
    smf_with(
        midly::Format::Parallel,
        Timing::Timecode(fps, subframes),
        &[vec![
            (0, Ev::Tempo(500_000)),
            (0, Ev::TimeSig(4, 2)),
            (0, Ev::NoteOn(0, 60, 90)),
            (division as u32, Ev::NoteOff(0, 60)),
            (division as u32, Ev::NoteOn(0, 64, 90)),
            (2 * division as u32, Ev::NoteOff(0, 64)),
            (2 * division as u32, Ev::NoteOn(0, 67, 90)),
            (3 * division as u32, Ev::NoteOff(0, 67)),
            (3 * division as u32, Ev::NoteOn(0, 72, 90)),
            (4 * division as u32, Ev::NoteOff(0, 72)),
        ]],
    )
}

#[test]
fn division_smpte_convertie_en_temps_puis_en_noires() {
    // `toMuseScoreTicks` (`importmidi_fraction.cpp:350-354`) laisse les ticks
    // SMPTE bruts : la carte des tempos n'est pas consultée. La quantification
    // répartit ensuite ces notes sur la grille ; on vérifie ici que les quatre
    // hauteurs arrivent dans l'ordre et à des attaques croissantes, sans la
    // conversion 120 BPM que le portage appliquait auparavant.
    let score = read_midi(&smpte(Fps::Fps30, 40, 1200)).expect("lecture SMPTE");
    let notes_smpte = notes(&score);
    let firsts: Vec<_> = notes_smpte.iter().step_by(2).map(|n| n.0).collect();
    assert_eq!(firsts, vec![60, 64, 67, 72]);
    let onsets: Vec<_> = notes_smpte.iter().map(|n| n.1).collect();
    assert!(onsets.windows(2).all(|w| w[0] <= w[1]));
    assert!(
        notes_smpte.last().unwrap().1 > Fraction::from_integer(1),
        "avec des ticks bruts, la dernière attaque dépasse une noire"
    );

    let drop = read_midi(&smpte(Fps::Fps29, 40, 1199)).expect("lecture drop-frame");
    assert!(!notes(&drop).is_empty(), "drop-frame 29,97");

    // Cas non concerné : une division métrique garde sa lecture actuelle
    // (480 ticks = une noire).
    let metrical = smf_with(
        midly::Format::Parallel,
        Timing::Metrical(midly::num::u15::new(480)),
        &[vec![
            (0, Ev::NoteOn(0, 60, 90)),
            (480, Ev::NoteOff(0, 60)),
            (480, Ev::NoteOn(0, 64, 90)),
            (960, Ev::NoteOff(0, 64)),
        ]],
    );
    let metrical = read_midi(&metrical).expect("lecture métrique");
    assert_eq!(
        notes(&metrical),
        vec![
            (60, Fraction::ZERO, Fraction::QUARTER),
            (64, Fraction::QUARTER, Fraction::QUARTER),
        ],
        "division métrique inchangée",
    );
}
