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
use scoretk::model::{
    tpc, ChordRest, Duration, Fraction, Measure, Note, Part, Score, Segment, SegmentItem,
    SegmentType, Track,
};
use scoretk::{read, write_playback, Format, Options};
use scoretk_engrave::{Engraver, Geometry, ItemKind};

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

/// A hyphen or melisma line is planned after the notation pass; it must not
/// inherit the last measure of the staff, nor lose the syllable's note id.
#[test]
fn lyric_lines_keep_their_measure_staff_and_note_ids() {
    let bytes = corpus("v4/copypastesymbollist-lyrics.mscx");
    let score = read(&bytes, Format::Mscx).expect("lecture MuseScore");
    let playback = write_playback(&score, &Options::default()).expect("playback");
    let xml_ids = musicxml_note_ids(&playback.musicxml);
    let engraver = Engraver::from_score(&score).expect("gravure");

    let mut syllables = Vec::new();
    let mut lines = Vec::new();
    for page in 0..engraver.page_count() {
        let list = engraver.page(page);
        for item in &list.items {
            if item.kind != ItemKind::Lyrics {
                continue;
            }
            let Some(id) = item.source.xml_id(&list.sources) else {
                continue;
            };
            let place = (
                id,
                item.source.system_index(),
                item.source.staff(),
                item.source.measure_ordinal(),
                item.origin().0,
            );
            match item.geometry {
                Geometry::Text { .. } => syllables.push(place),
                Geometry::Rect { .. } => lines.push(place),
                _ => {}
            }
        }
    }
    assert!(!syllables.is_empty(), "aucune syllabe gravée");
    assert!(!lines.is_empty(), "aucun trait de parole gravé");
    for (id, system, staff, measure, x) in lines {
        assert!(xml_ids.contains(&id), "trait de parole inconnu : {id}");
        assert!(staff.is_some() && measure.is_some(), "trait sans placement");
        let owner = syllables
            .iter()
            .find(|(s_id, s_system, s_staff, s_measure, s_x)| {
                s_id == &id
                    && s_system == &system
                    && s_staff == &staff
                    && s_measure == &measure
                    && *s_x <= x
            });
        assert!(owner.is_some(), "trait {id} sans syllabe source");
    }
}

/// `mergeMatchingRests` skips one of two identical rests and
/// `resolveRestVSChord`/`resolveRestVSRest` move rests vertically. Neither
/// changes the identity stamped on the surviving glyph.
#[test]
fn merged_and_moved_rests_keep_their_source() {
    let bytes = corpus("v4/mmrest.mscx");
    let mut score = read(&bytes, Format::Mscx).expect("lecture MuseScore");
    let staff = score.staves[0].id;
    // Add a second, identical voice to each measure rest: the staff property
    // then skips one glyph out of every pair. Unfold the corpus' multi-measure
    // rests so every source measure is actually engraved.
    for measure in &mut score.measures {
        measure.multi_measure_rest = None;
        for segment in &mut measure.segments {
            let Some(rest) = segment.chord_rest(Track::new(staff, 0)) else {
                continue;
            };
            if !rest.is_rest() {
                continue;
            }
            let duration = rest.duration;
            segment.set_element(
                Track::new(staff, 1),
                SegmentItem::ChordRest(ChordRest::rest(duration)),
            );
        }
    }
    let playback = write_playback(&score, &Options::default()).expect("playback");
    let xml_ids = musicxml_note_ids(&playback.musicxml);
    let rest_ids = |score: &Score| -> Vec<String> {
        let engraver = Engraver::from_score(score).expect("gravure");
        let mut ids = Vec::new();
        for page in 0..engraver.page_count() {
            let list = engraver.page(page);
            for item in &list.items {
                if item.kind == ItemKind::Rest {
                    ids.push(
                        item.source
                            .xml_id(&list.sources)
                            .expect("silence sans identifiant"),
                    );
                }
            }
        }
        ids
    };
    let drawn = rest_ids(&score);
    assert!(!drawn.is_empty(), "aucun silence gravé");
    let mut merged = score.clone();
    merged.staves[0].merge_matching_rests = true;
    let after_merge = rest_ids(&merged);
    assert!(
        after_merge.len() < drawn.len(),
        "mergeMatchingRests should drop one rest per pair"
    );
    for id in after_merge {
        assert!(xml_ids.contains(&id), "silence fusionné inconnu : {id}");
    }

    // A rest sharing its segment with the other voice's chord goes through
    // the collision passes; it keeps its identity while being moved.
    let polyphonic = || {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Voice", "Vox"));
        let staff = score.add_staff(part);
        let staff_count = score.staves.len();
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, staff_count);
        let note = score.add_note(Note::new(60, tpc::C));
        let mut chord = ChordRest::chord(Duration::quarter());
        chord.chord_mut().expect("accord").notes.push(note);
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        segment.set_element(
            Track::new(staff, 1),
            SegmentItem::ChordRest(ChordRest::rest(Duration::quarter())),
        );
        measure.add_segment(segment);
        score.add_measure(measure);
        score
    };
    let score = polyphonic();
    let playback = write_playback(&score, &Options::default()).expect("playback");
    let xml_ids = musicxml_note_ids(&playback.musicxml);
    let engraver = Engraver::from_score(&score).expect("gravure");
    let list = engraver.page(0);
    let staff_lines: Vec<i32> = list
        .items
        .iter()
        .filter(|item| item.kind == ItemKind::StaffLine)
        .map(|item| item.origin().1)
        .collect();
    assert!(staff_lines.len() >= 2, "portée absente de la page");
    let sp = staff_lines[1] - staff_lines[0];
    let mut ids = Vec::new();
    let mut ys = Vec::new();
    for item in &list.items {
        if item.kind == ItemKind::Rest {
            ids.push(
                item.source
                    .xml_id(&list.sources)
                    .expect("silence sans identifiant"),
            );
            ys.push(item.origin().1);
        }
    }
    assert!(!ids.is_empty(), "aucun silence dans la mesure polyphonique");
    assert!(
        xml_ids.contains(&ids[0]),
        "silence déplacé inconnu : {}",
        ids[0]
    );
    // Voice 1 alone would put the quarter rest at `staff_top + 1.5 sp`;
    // `resolveRestVSChord` must push it further away from the chord.
    assert!(
        ys[0] != staff_lines[0] + 3 * sp / 2,
        "le silence devrait être déplacé par la résolution de collision"
    );
}
