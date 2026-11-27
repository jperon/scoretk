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

//! Chaque tête de note gravée porte le même identifiant que la `<note>` du
//! MusicXML identifié produit par `write_playback` (et donc que
//! `timeline.notes[].xmlId`). Voir `docs/agents/timemap.md` §3.

#![cfg(all(
    feature = "engrave",
    feature = "musescore-import",
    feature = "musicxml-export",
    feature = "midi-export"
))]

use std::collections::BTreeSet;

use quick_xml::events::Event;
use quick_xml::Reader;
use scoretk::{read, write_playback, Format, Options};
use scoretk_engrave::{Engraver, ItemKind};

fn corpus(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("lecture de {} : {error}", path.display()))
}

fn musicxml_note_ids(xml: &[u8]) -> BTreeSet<String> {
    let mut reader = Reader::from_str(std::str::from_utf8(xml).expect("UTF-8"));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element) | Event::Start(element))
                if element.name().as_ref() == "note" =>
            {
                for attribute in element.attributes().flatten() {
                    if attribute.key.as_ref() == "id" {
                        ids.insert(attribute.value.into_owned());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML invalide : {error}"),
        }
        buffer.clear();
    }
    ids
}

fn engraved_note_ids(bytes: &[u8]) -> (BTreeSet<String>, BTreeSet<String>) {
    let score = read(bytes, Format::Mscx).expect("lecture MuseScore");
    let engraver = Engraver::from_score(&score).expect("gravure");
    let mut heads = BTreeSet::new();
    let mut timemap_notes = BTreeSet::new();
    let playback = write_playback(&score, &Options::default()).expect("playback");
    for note in &playback.timemap.notes {
        timemap_notes.insert(note.xml_id());
    }
    for page in 0..engraver.page_count() {
        let list = engraver.page(page);
        for item in &list.items {
            if item.kind == ItemKind::Notehead {
                let id = item
                    .source
                    .xml_id(&list.sources)
                    .expect("tête de note sans identifiant");
                heads.insert(id);
            }
        }
    }
    (heads, timemap_notes)
}

#[test]
fn noteheads_match_stable_musicxml_and_timemap_ids() {
    for file in [
        "v4/voiceswitching.mscx",
        "v4/changevisibility.mscx",
        "v3/moonlight.mscx",
    ] {
        let bytes = corpus(file);
        let (heads, timemap_notes) = engraved_note_ids(&bytes);
        let score = read(&bytes, Format::Mscx).expect("lecture MuseScore");
        let xml = write_playback(&score, &Options::default()).expect("playback");
        let xml_ids = musicxml_note_ids(&xml.musicxml);
        assert!(!heads.is_empty(), "aucune tête de note dans {file}");
        let unknown: Vec<_> = heads.difference(&xml_ids).collect();
        assert!(
            unknown.is_empty(),
            "{file}: ids absents du MusicXML : {unknown:?}"
        );
        // Tied continuation notes have no MIDI attack, but every attack must
        // have a drawn notehead unless the source note is explicitly hidden.
        let missing: Vec<_> = timemap_notes.difference(&heads).collect();
        assert!(
            missing.is_empty(),
            "{file}: attaques sans tête de note : {missing:?}"
        );
    }
}
