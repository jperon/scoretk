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

//! `FF 04` is the standard SMF carrier for the instrument name.

use std::fs;
use std::path::Path;

use midly::{MetaMessage, Smf, TrackEventKind};
use scoretk_midi_export::{write_midi, write_midi_with, MidiOptions};
use scoretk_midi_import::{read_midi, read_midi_with, ImportOptions};
use scoretk_musescore_import::read;
use scoretk_musescore_import::MsczContainer;

fn score() -> scoretk_core::model::Score {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/v3/barlinedelete.mscx");
    let bytes = fs::read(path).expect("corpus score");
    let container = MsczContainer::from_bytes(&bytes).expect("container");
    read::read_score(&container).expect("MuseScore import")
}

fn instrument_names(bytes: &[u8]) -> Vec<String> {
    let smf = Smf::parse(bytes).expect("valid SMF");
    let mut result = Vec::new();
    for track in &smf.tracks {
        for event in track {
            if let TrackEventKind::Meta(MetaMessage::InstrumentName(data)) = event.kind {
                result.push(
                    String::from_utf8_lossy(data)
                        .trim_end_matches('\0')
                        .to_owned(),
                );
            }
        }
    }
    result.sort();
    result
}

#[test]
fn instrument_name_uses_ff04_in_full_support_only() {
    let source = score();
    let expected: Vec<String> = source
        .parts
        .iter()
        .map(|part| part.instrument.track_name.clone())
        .filter(|name| !name.is_empty())
        .collect();
    assert!(!expected.is_empty(), "fixture instrument name");

    let default = write_midi(&source).expect("default export");
    assert!(
        instrument_names(&default).is_empty(),
        "4.7.4 writes no FF 04"
    );
    // The default reader may invent a GM catalogue name: that is the 4.7.4
    // path which full support replaces with `FF 04`.
    let _ = read_midi(&default).expect("default import");

    let written = write_midi_with(
        &source,
        MidiOptions {
            full_support: true,
            ..Default::default()
        },
    )
    .expect("full support export");
    assert_eq!(instrument_names(&written), expected);
    let imported = read_midi_with(
        &written,
        &ImportOptions {
            full_support: true,
            ..Default::default()
        },
    )
    .expect("full support import");
    assert_eq!(
        imported
            .parts
            .iter()
            .map(|part| part.instrument.track_name.clone())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>(),
        expected
    );
}
