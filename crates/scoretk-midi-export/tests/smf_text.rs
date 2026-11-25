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

//! `FF 01`/`FF 02`/`FF 06` are optional: the default export stays faithful to
//! MuseScore 4.7.4, the additive option writes them and the additive reader
//! restores them (`docs/agents/midi.md` §"SMF item declaration audit").

use std::fs;
use std::path::Path;

use midly::{MetaMessage, Smf, TrackEventKind};
use scoretk_core::model::Annotation;
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

fn meta_counts(bytes: &[u8]) -> (usize, usize, usize) {
    let smf = Smf::parse(bytes).expect("valid SMF");
    let mut text = 0;
    let mut copyright = 0;
    let mut marker = 0;
    for track in &smf.tracks {
        for event in track {
            match event.kind {
                TrackEventKind::Meta(MetaMessage::Text(_)) => text += 1,
                TrackEventKind::Meta(MetaMessage::Copyright(_)) => copyright += 1,
                TrackEventKind::Meta(MetaMessage::Marker(_)) => marker += 1,
                _ => {}
            }
        }
    }
    (text, copyright, marker)
}

fn annotation_count(score: &scoretk_core::model::Score, staff_text: bool) -> usize {
    let mut result = 0;
    for measure in &score.measures {
        for segment in &measure.segments {
            for entry in &segment.annotations {
                let matches = if staff_text {
                    matches!(entry.annotation, Annotation::StaffText(_))
                } else {
                    matches!(entry.annotation, Annotation::RehearsalMark(_))
                };
                result += usize::from(matches);
            }
        }
    }
    result
}

#[test]
fn smf_text_is_opt_in_and_round_trips() {
    let source = score();
    let source_text = annotation_count(&source, true);
    let source_marks = annotation_count(&source, false);
    assert!(source_text > 0 && source_marks > 0, "corpus fixture");

    let default = write_midi(&source).expect("default export");
    assert_eq!(
        meta_counts(&default),
        (0, 0, 0),
        "MuseScore 4.7.4 writes no generic text meta"
    );

    let options = MidiOptions {
        smf_text: true,
        ..Default::default()
    };
    let written = write_midi_with(&source, options).expect("SMF text export");
    let (text, copyright, marker) = meta_counts(&written);
    assert!(
        text > 0 && marker > 0,
        "missing text/marker metas: text={text} copyright={copyright} marker={marker}"
    );

    let imported = read_midi_with(
        &written,
        &ImportOptions {
            smf_text: true,
            ..Default::default()
        },
    )
    .expect("SMF text import");
    assert_eq!(annotation_count(&imported, true), source_text);
    assert_eq!(annotation_count(&imported, false), source_marks);
    assert_eq!(imported.metadata.work_title, source.metadata.work_title);

    // The default reader still treats FF 01 as lyrics (MuseScore behaviour).
    let plain = read_midi(&written).expect("default import");
    assert_eq!(annotation_count(&plain, true), 0);
}
