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

//! A chord whose only note is discarded as too short must not panic.

use scoretk_core::model::Fraction;
use scoretk_midi_import::chord::{remove_overlapping_notes, MTrack, MidiChord, MidiNote};

#[test]
fn discarding_the_last_note_does_not_panic() {
    // Exact timing pattern of `Toto/Don't Chain My Heart.mid` from Lakh: two
    // same-pitch chords in the same voice 1/192 apart (below the 1/128 minimum
    // duration). The first note is truncated and removed. A cached note count
    // then made the loop index `notes[0]` on an empty vector and panic.
    let on_time1 = Fraction::new(7221, 64);
    let on_time2 = Fraction::new(677, 6);
    let mut track = MTrack::default();
    for on_time in [on_time1, on_time2] {
        let note = MidiNote::new(43, 80, on_time + Fraction::new(1, 16), on_time);
        track.chords.insert(
            on_time,
            MidiChord {
                notes: vec![note],
                voice: 0,
                ..MidiChord::default()
            },
        );
    }

    remove_overlapping_notes(&mut track);
    assert_eq!(track.chords.len(), 1, "the truncated chord must be removed");
    let remaining = track.chords.iter().next().expect("one chord left");
    assert_eq!(remaining.0, on_time2, "the later chord survives");
    assert_eq!(remaining.1.notes.len(), 1, "the later note survives");
    assert_eq!(remaining.1.notes[0].pitch, 43, "pitch preserved");
}
