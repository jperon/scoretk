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

//! An implicit 4/4 is written explicitly in full support (`FF 58 04 nn dd`).

use std::fs;
use std::path::Path;

use midly::{MetaMessage, Smf, TrackEventKind};
use scoretk_midi_export::{write_midi, write_midi_with, MidiOptions};
use scoretk_musescore_import::read;
use scoretk_musescore_import::MsczContainer;

fn score() -> scoretk_core::model::Score {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/v4/repeat68.mscx");
    let bytes = fs::read(path).expect("corpus score");
    let container = MsczContainer::from_bytes(&bytes).expect("container");
    read::read_score(&container).expect("MuseScore import")
}

fn time_signatures(bytes: &[u8]) -> Vec<(u8, u8)> {
    let smf = Smf::parse(bytes).expect("valid SMF");
    let mut result = Vec::new();
    for track in &smf.tracks {
        for event in track {
            if let TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, exponent, _, _)) =
                event.kind
            {
                result.push((numerator, exponent));
            }
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn implicit_four_four_is_written_only_in_full_support() {
    let source = score();
    let default = write_midi(&source).expect("default export");
    assert!(
        time_signatures(&default).is_empty(),
        "4.7.4 leaves 4/4 implicit"
    );
    let written = write_midi_with(
        &source,
        MidiOptions {
            full_support: true,
            ..Default::default()
        },
    )
    .expect("full support export");
    assert_eq!(time_signatures(&written), vec![(4, 2)]);
}
