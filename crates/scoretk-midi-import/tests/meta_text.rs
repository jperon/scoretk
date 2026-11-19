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

//! Guards against malformed MusicXML caused by control characters in MIDI
//! meta text (NUL-terminated track names from the Lakh corpus).

mod common;

use common::{smf, Ev};

#[test]
fn nul_terminated_track_names_produce_well_formed_musicxml() {
    let bytes = smf(&[
        vec![
            (0, Ev::Tempo(500_000)),
            (0, Ev::TimeSig(4, 2)), // 4/4
        ],
        vec![
            (0, Ev::TrackName(b"Piano\0")),
            (0, Ev::Program(0, 0)),
            (0, Ev::NoteOn(0, 60, 80)),
            (0, Ev::NoteOn(0, 64, 80)),
            (480, Ev::NoteOff(0, 60)),
            (480, Ev::NoteOff(0, 64)),
            (960, Ev::NoteOn(0, 67, 80)),
            (960, Ev::Lyric(b"la\0")),
            (1440, Ev::NoteOff(0, 67)),
            (1920, Ev::NoteOn(0, 69, 80)),
            (2160, Ev::NoteOff(0, 69)),
        ],
        vec![
            (0, Ev::TrackName(b"Strings\0")),
            (0, Ev::Program(1, 48)),
            (960, Ev::NoteOn(1, 72, 80)),
            (1440, Ev::NoteOff(1, 72)),
            (1920, Ev::NoteOn(1, 74, 80)),
            (2400, Ev::NoteOff(1, 74)),
        ],
    ]);

    let score = scoretk_midi_import::read_midi(&bytes).expect("MIDI import");
    assert_eq!(score.parts.len(), 2, "parts");
    assert_eq!(score.measures.len(), 2, "measures");
    assert_eq!(score.notes.len(), 6, "notes");
    let mut pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
    pitches.sort_unstable();
    assert_eq!(pitches, vec![60, 64, 67, 69, 72, 74], "pitches");
    assert!(
        score.parts.iter().all(|part| !part.name.contains('\0')),
        "a NUL survived in a part name"
    );

    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("MusicXML export");
    assert!(
        !xml.as_bytes().contains(&0),
        "NUL byte in the exported MusicXML: {xml}"
    );
    let mut reader = quick_xml::Reader::from_str(&xml);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("malformed MusicXML: {error}"),
        }
    }
    assert!(xml.contains("Piano"), "track name missing: {xml}");
    assert!(xml.contains("Strings"), "track name missing: {xml}");
}

#[test]
fn clean_track_names_are_unchanged() {
    // Non-affected case: ordinary names and XML-allowed whitespace must pass
    // through byte for byte.
    let bytes = smf(&[vec![
        (0, Ev::TrackName(b"Piano \"Grand\"\tA\nB")),
        (0, Ev::Program(0, 0)),
        (0, Ev::NoteOn(0, 60, 80)),
        (480, Ev::NoteOff(0, 60)),
    ]]);
    let score = scoretk_midi_import::read_midi(&bytes).expect("MIDI import");
    assert_eq!(score.notes.len(), 1, "notes");
    assert!(
        score
            .parts
            .iter()
            .any(|part| part.name.contains("Piano \"Grand\"")),
        "name lost"
    );
}
