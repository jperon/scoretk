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

//! A per-note channel restores the Note On status nibble on export.

use std::fs;
use std::path::Path;

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_midi_export::write_midi;
use scoretk_musescore_import::read;
use scoretk_musescore_import::MsczContainer;

fn score() -> scoretk_core::model::Score {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/v3/barlinedelete.mscx");
    let bytes = fs::read(path).expect("corpus score");
    let container = MsczContainer::from_bytes(&bytes).expect("container");
    read::read_score(&container).expect("MuseScore import")
}

fn note_on_channels(bytes: &[u8]) -> Vec<u8> {
    let smf = Smf::parse(bytes).expect("valid SMF");
    let mut result = Vec::new();
    for track in &smf.tracks {
        for event in track {
            if let TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { vel, .. },
            } = event.kind
            {
                if vel.as_int() > 0 {
                    result.push(channel.as_int());
                }
            }
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn note_with_a_recorded_channel_is_written_with_that_status_nibble() {
    let mut source = score();
    let note = source.notes.first_mut().expect("a note in the fixture");
    note.playback.midi_channel = Some(11);
    let bytes = write_midi(&source).expect("default export");
    let channels = note_on_channels(&bytes);
    assert!(
        channels.contains(&11),
        "the status nibble must carry channel 11: {channels:?}"
    );

    let untouched = write_midi(&score()).expect("unmodified export");
    assert!(
        !note_on_channels(&untouched).contains(&11),
        "default bytes unchanged"
    );
}
