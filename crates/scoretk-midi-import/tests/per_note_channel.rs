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

//! `[MIDI §Channel Voice]` carries the channel in each Note On status nibble.

mod common;

use scoretk_core::model::{ChordRestKind, Score, SegmentItem};
use scoretk_midi_import::{read_midi, read_midi_with, ImportOptions};

use common::{smf, Ev};

fn channels(score: &Score) -> Vec<u8> {
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
                for id in &chord.notes {
                    if let Some(channel) = score.notes[id.index()].playback.midi_channel {
                        result.push(channel);
                    }
                }
            }
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn full_support_keeps_the_note_on_channel() {
    let bytes = smf(&[vec![
        (0, Ev::Program(0, 0)),
        (0, Ev::Program(1, 0)),
        (0, Ev::NoteOn(0, 60, 80)),
        (0, Ev::NoteOn(1, 62, 80)),
        (480, Ev::NoteOff(0, 60)),
        (480, Ev::NoteOff(1, 62)),
    ]]);
    let default = read_midi(&bytes).expect("default import");
    assert!(
        channels(&default).is_empty(),
        "4.7.4 has no per-note channel"
    );

    let full = read_midi_with(
        &bytes,
        &ImportOptions {
            full_support: true,
            ..Default::default()
        },
    )
    .expect("full support import");
    assert_eq!(channels(&full), vec![0, 1]);
}
