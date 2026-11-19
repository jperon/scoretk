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

use common::{Ev, smf_with};

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
    // 30 images/s × 40 ticks/image = 1200 ticks/s ; à 120 BPM une seconde
    // vaut deux noires (Fraction 1/2 de ronde), pas 1200/480 noires.
    let score = read_midi(&smpte(Fps::Fps30, 40, 1200)).expect("lecture SMPTE");
    assert_eq!(score.measures.len(), 2, "mesures");
    assert_eq!(score.notes.len(), 4, "notes");
    assert_eq!(
        notes(&score),
        vec![
            (60, Fraction::ZERO, Fraction::HALF),
            (64, Fraction::HALF, Fraction::HALF),
            (67, Fraction::from_integer(1), Fraction::HALF),
            (72, Fraction::new(3, 2), Fraction::HALF),
        ],
        "hauteur, attaque et durée en noires",
    );

    // 29,97 images/s (30 drop-frame) × 40 = 1199 ticks/s après arrondi ;
    // 1199 ticks valent toujours une seconde, donc deux noires.
    let drop = read_midi(&smpte(Fps::Fps29, 40, 1199)).expect("lecture drop-frame");
    assert_eq!(
        notes(&drop),
        vec![
            (60, Fraction::ZERO, Fraction::HALF),
            (64, Fraction::HALF, Fraction::HALF),
            (67, Fraction::from_integer(1), Fraction::HALF),
            (72, Fraction::new(3, 2), Fraction::HALF),
        ],
        "drop-frame 29,97",
    );

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
