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

//! Guards the per-bar range queries used by tuplet detection, duration
//! simplification and voice separation on a dense multi-bar, multi-track SMF.

mod common;

use common::{chords, smf, Ev};

#[test]
fn dense_bars_keep_tuplets_and_note_boundaries() {
    // Twelve 4/4 bars: eleven are filled with sixteenth notes, one holds a
    // clean triplet (3 x 640 ticks, a non-binary grid). Two tracks exercise
    // the per-track bar window used by tuplet detection.
    let mut tracks = Vec::new();
    for &channel in &[0_u8, 1_u8] {
        let mut events = vec![(0, Ev::Program(channel, 0))];
        for bar in 0..12_u32 {
            let base = bar * 1920;
            if bar == 6 {
                for k in 0..3_u32 {
                    let at = base + k * 640;
                    events.push((at, Ev::NoteOn(channel, 60 + channel + k as u8, 90)));
                    events.push((at + 640, Ev::NoteOff(channel, 60 + channel + k as u8)));
                }
            } else {
                for k in 0..16_u32 {
                    let at = base + k * 120;
                    events.push((at, Ev::NoteOn(channel, 60 + channel, 90)));
                    events.push((at + 120, Ev::NoteOff(channel, 60 + channel)));
                }
            }
        }
        tracks.push(events);
    }
    let bytes = smf(&tracks);

    let score = scoretk_midi_import::read_midi(&bytes).expect("MIDI import");
    assert_eq!(score.parts.len(), 2, "parts");
    assert_eq!(score.measures.len(), 12, "measures");
    assert_eq!(
        score.notes.len(),
        2 * (11 * 16 + 3),
        "notes across dense bars"
    );
    assert!(!score.tuplets.is_empty(), "triplet not detected");
    let notes = chords(&score);
    assert_eq!(notes.len(), 2 * (11 * 16 + 3), "chords");
    // A sixteenth note is 120 ticks in 1/1920-whole units (480 per quarter).
    let sixteenths = notes.iter().filter(|(_, ticks)| *ticks == 120).count();
    assert_eq!(sixteenths, 2 * 11 * 16, "sixteenth-note durations");
    // The triplet written on a 640-tick grid is modelled as a half note (960
    // ticks) under a 3:2 tuplet, which preserves its sounding length.
    let triplet = notes.iter().filter(|(_, ticks)| *ticks == 960);
    assert_eq!(triplet.clone().count(), 6, "triplet note count");
    let pitches: Vec<u8> = triplet
        .flat_map(|(pitches, _)| pitches.iter().copied())
        .collect();
    assert_eq!(pitches, vec![60, 61, 61, 62, 62, 63], "triplet pitches");
}
