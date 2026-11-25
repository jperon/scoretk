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

//! Facade wiring of GABC: extension, content detection and round trip.

use scoretk::model::Extension;
use scoretk::{read, write, Format, Options};
use std::fs;
use std::path::Path;

const GABC: &str = "% a comment\nname: Veni Sancte Spiritus;\nlanguage: latin;\nmode: 2;\n%%\n(c3)\nVE(g.)ni(h') (,)\n";

#[test]
fn gabc_is_recognised_by_extension_and_content() {
    assert_eq!(Format::from_extension("gabc"), Some(Format::Gabc));
    assert_eq!(Format::from_extension("GABC"), Some(Format::Gabc));
    assert_eq!(Format::detect(GABC.as_bytes()), Some(Format::Gabc));
    // Comments before `name:` are allowed by Gregorio's examples.
    assert!(GABC.starts_with('%'));
    assert_eq!(Format::detect(b"%abc-2.1\nX:1\n"), Some(Format::Abc));
    assert_eq!(Format::detect(b""), None);
}

#[test]
fn gabc_round_trip_serialises_the_payload() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let bytes = write(&score, Format::Gabc, &Options::default()).expect("GABC export");
    let written = String::from_utf8(bytes).expect("UTF-8");
    // The leading comment is not a modelled field: it must not come back.
    // A byte-identical output would mean the writer copied its input.
    assert!(!written.contains("a comment"), "{written}");
    assert!(
        written.starts_with("name: Veni Sancte Spiritus;\n"),
        "{written}"
    );
    let again = read(written.as_bytes(), Format::Gabc).expect("second import");
    let reference = scoretk::model::inventory(&score);
    let candidate = scoretk::model::inventory(&again);
    let coverage = scoretk::model::compare(&reference, &candidate, scoretk::model::Layers::ALL);
    assert_eq!(
        (
            coverage.total.reference,
            coverage.total.matched,
            coverage.total.extra
        ),
        (reference.items.len(), reference.items.len(), 0),
        "the structured inventory changed"
    );
}

/// Minimal SMF scan used as an oracle-free check of GABC → MIDI: count the
/// note-ons with a positive velocity and the lyric/text metas.
fn midi_events(data: &[u8]) -> (usize, Vec<String>) {
    fn vlq(data: &[u8], at: &mut usize) -> usize {
        let mut value = 0;
        loop {
            let byte = data[*at];
            *at += 1;
            value = (value << 7) | usize::from(byte & 0x7f);
            if byte & 0x80 == 0 {
                return value;
            }
        }
    }
    let mut at = 14;
    let mut notes = 0;
    let mut words = Vec::new();
    while at < data.len() {
        assert_eq!(&data[at..at + 4], b"MTrk");
        let end = at + 8 + u32::from_be_bytes(data[at + 4..at + 8].try_into().unwrap()) as usize;
        at += 8;
        let mut running = 0u8;
        while at < end {
            vlq(data, &mut at);
            let status = data[at];
            if status & 0x80 != 0 {
                running = status;
                at += 1;
            }
            if running == 0xff {
                let meta = data[at];
                at += 1;
                let len = vlq(data, &mut at);
                if meta == 1 || meta == 5 {
                    words.push(String::from_utf8_lossy(&data[at..at + len]).into_owned());
                }
                at += len;
            } else if matches!(running & 0xf0, 0x80 | 0x90 | 0xa0 | 0xb0 | 0xe0) {
                let velocity = data[at + 1];
                if running & 0xf0 == 0x90 && velocity > 0 {
                    notes += 1;
                }
                at += 2;
            } else if matches!(running & 0xf0, 0xc0 | 0xd0) {
                at += 1;
            } else {
                panic!("unsupported MIDI status {running:#x}");
            }
        }
        at = end;
    }
    (notes, words)
}

#[test]
fn gabc_to_midi_keeps_one_note_per_letter_and_the_syllables() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    assert_eq!(
        score.notes.len(),
        2,
        "the derived projection has two letters"
    );
    let midi = write(&score, Format::Midi, &Options::default()).expect("MIDI export");
    let (notes, words) = midi_events(&midi);
    assert_eq!(notes, 2, "one MIDI note-on per GABC pitch letter");
    assert!(
        words.iter().any(|word| word.contains("VE")),
        "the syllable text reached the MIDI lyric meta: {words:?}"
    );
}

/// `Gabc.tex:481-489,512-540` : `x` est un glyphe d'alteration, pas une note
/// chantée ; il apply un bémol à la note suivante de la même hauteur.
#[test]
fn an_alteration_is_not_a_note_and_changes_pitch_and_musicxml_alter() {
    const ALTER: &str = "name:t;\n%%\n(c4) A(gixi)ve(h_g.) (::)\n";
    let score = read(ALTER.as_bytes(), Format::Gabc).expect("GABC import");
    let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
    assert_eq!(
        pitches,
        [55, 58, 57, 55],
        "the alteration glyph emits no note and flattens the following `i`"
    );
    let xml = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert!(xml.contains("<alter>-1</alter>"), "{xml}");
    assert!(
        !xml.contains("<notehead>x</notehead>"),
        "an alteration is not a cross notehead: {xml}"
    );
    // L'aller-retour L6 doit conserver le glyphe d'alteration lui-même.
    let gabc = String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC"))
        .expect("UTF-8 GABC");
    assert!(gabc.contains("gixi"), "{gabc}");
    let again = read(gabc.as_bytes(), Format::Gabc).expect("second import");
    assert_eq!(again.notes.len(), score.notes.len());
}

/// Une mesure dérivée par barre de séparation réelle, jamais par syllabe
/// (`Gabc.tex:801-845`), sans mesure métrique affichée.
#[test]
fn gabc_measures_follow_separation_bars_not_syllables() {
    const BARS: &str = "name:t;\n%%\n(c4) A(g)h(i) (,) B(g)h(i) (;) C(g)h(i) (:) D(g)h(i) (::)\n";
    let score = read(BARS.as_bytes(), Format::Gabc).expect("GABC import");
    assert_eq!(
        score.measures.len(),
        4,
        "one measure per separation bar, not one per syllable group"
    );
    assert_eq!(score.notes.len(), 8);
    let xml = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert!(
        xml.contains("<time print-object=\"no\"><senza-misura/></time>")
            || xml.contains("<senza-misura"),
        "chant must be explicitly unmetered: {xml}"
    );
    assert!(!xml.contains("<beats>"), "no displayed meter: {xml}");
    // `Gabc.tex:801-821` : minima (quarter bar) → tick, minor (half bar) →
    // short, maior (full bar) → light-light, finalis → light-heavy.
    for style in ["tick", "short", "light-light", "light-heavy"] {
        assert!(
            xml.contains(&format!("<bar-style>{style}</bar-style>")),
            "missing bar-style {style}: {xml}"
        );
    }
    assert!(xml.contains("<breath-mark"), "bars are breaths: {xml}");
    let playback = scoretk::write_playback(&score, &Options::default()).expect("playback");
    assert_eq!(playback.timemap.pauses.len(), 4, "one MIDI breath per bar");
    assert_eq!(playback.timemap.notes.len(), 8);
}

/// `Gabc.tex:742-797` : punctum mora × 2, épisème horizontal × 1,5 ; les
/// deux ensemble donnent une blanche pointée. La durée de base est la noire.
#[test]
fn gabc_durations_reach_musicxml_and_midi() {
    const DUR: &str = "name:t;\n%%\n(c4) A(g.)h(i_)B(g._) (::)\n";
    let score = read(DUR.as_bytes(), Format::Gabc).expect("GABC import");
    let playback = scoretk::write_playback(&score, &Options::default()).expect("playback");
    let ticks: Vec<i64> = playback
        .timemap
        .notes
        .iter()
        .map(|note| note.duration_ticks)
        .collect();
    // Le `NoteOff` SMF est à `NoteOn + durée - 1` (gate MIDI).
    assert_eq!(ticks, [959, 719, 1439], "half, dotted quarter, dotted half");
    let xml = String::from_utf8(playback.musicxml).expect("UTF-8 MusicXML");
    assert!(xml.contains("<type>half</type>"), "{xml}");
    assert!(xml.contains("<dot/>"), "episema lengthens by 1.5: {xml}");
}

/// Positions source UTF-16 : identifiants MusicXML et carte des temps sont
/// calculés par la même fonction du noyau, donc exactement égaux.
#[test]
fn gabc_source_positions_reach_musicxml_ids_and_timeline() {
    const SOURCE: &str = "name:t;\n%%\n(c4) é(fgh)b(gixi) (::)\n";
    let score = read(SOURCE.as_bytes(), Format::Gabc).expect("GABC import");
    let playback = scoretk::write_playback(&score, &Options::default()).expect("playback");
    let xml = String::from_utf8(playback.musicxml.clone()).expect("UTF-8 MusicXML");
    let expected = [
        ("m0-s0-v0-c0-n0", 18, 19),
        ("m0-s0-v0-c1-n0", 19, 20),
        ("m0-s0-v0-c2-n0", 20, 21),
        ("m0-s0-v0-c3-n0", 24, 25),
        ("m0-s0-v0-c4-n0", 27, 28),
    ];
    assert_eq!(playback.timemap.notes.len(), expected.len());
    for ((id, offset, end), note) in expected.iter().zip(&playback.timemap.notes) {
        assert_eq!(note.source_offset, Some(*offset));
        assert_eq!(note.source_end, Some(*end));
        assert_eq!(note.xml_id(), format!("{id}-o{offset}-e{end}"));
        assert!(
            xml.contains(&format!("id=\"{}-o{offset}-e{end}\"", id)),
            "the identifier must be in the MusicXML: {xml}"
        );
    }
}

/// A monophonic MuseScore example from the versioned corpus: one staff, four
/// notes and one lyric syllable. It has no L6 payload, so it exercises the
/// score → GABC projection rather than the payload serializer.
#[test]
fn a_score_without_a_gregorian_payload_is_written_as_chant() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3")
        .join("lyricsline01.mscx");
    let source = fs::read(&path).expect("read the monophonic corpus score");
    let score = read(&source, Format::Mscx).expect("MuseScore import");
    assert!(
        score.extensions.format.is_none(),
        "no L6 payload in the input"
    );
    let bytes = write(&score, Format::Gabc, &Options::default()).expect("GABC export");
    let text = String::from_utf8(bytes).expect("UTF-8");
    assert!(text.starts_with("name: "), "{text}");
    assert!(text.contains("\n%%\n"), "{text}");

    let again = read(text.as_bytes(), Format::Gabc).expect("written GABC parses back");
    let Some(Extension::Gregorian(gregorian)) = &again.extensions.format else {
        panic!("the output carries a Gregorian payload");
    };
    assert_eq!(gregorian.syllables.len(), score.notes.len());
    assert!(
        gregorian
            .syllables
            .iter()
            .any(|syllable| !syllable.text.is_empty()),
        "lyrics survived"
    );
    assert!(gregorian
        .syllables
        .iter()
        .all(|syllable| syllable.notes.len() <= 1));
    assert!(gregorian
        .syllables
        .last()
        .is_some_and(|syllable| !syllable.bars.is_empty()));
}
