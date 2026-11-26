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
    // The leading `%` comment is now an ordered header item (docs/agents/
    // gabc.md §20), so it must come back *in place*. Byte identity is no
    // longer a sign of a raw copy: the model has no source-text field and
    // `scoretk-gabc-export::tests::payload_has_no_raw_source_field` checks
    // that compile-time invariant.
    assert!(written.starts_with("% a comment\nname: "), "{written}");
    assert_eq!(written, GABC, "the structured round trip must be exact");
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

/// Option additive `gabcModernClef` : clé de sol (avec `8vb` pour un registre
/// bas) et 5 lignes ; défaut = clé d'ut et 4 lignes (`Gabc.tex:847-870`).
#[test]
fn gabc_modern_clef_option_is_additive() {
    const SOURCE: &str = "name:t;\n%%\n(c4) A(g)h(i) (::)\n";
    let score = read(SOURCE.as_bytes(), Format::Gabc).expect("GABC import");
    let default = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert!(default.contains("<sign>C</sign>"), "{default}");
    assert!(
        default.contains("<staff-lines>4</staff-lines>"),
        "{default}"
    );
    let modern_options = Options {
        gabc_modern_clef: true,
        ..Options::default()
    };
    let modern = String::from_utf8(
        write(&score, Format::MusicXml, &modern_options).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert!(modern.contains("<sign>G</sign>"), "{modern}");
    assert!(
        modern.contains("<clef-octave-change>-1</clef-octave-change>"),
        "a low Gregorian range needs 8vb: {modern}"
    );
    assert!(!modern.contains("<staff-lines>4</staff-lines>"), "{modern}");
}

/// `Gabc.tex:1088-1091` : `[alt:…]` est un texte au-dessus de la portée.
#[test]
fn gabc_above_text_becomes_musicxml_words() {
    const ALT: &str = "name:t;\n%%\n(c4) x(g[alt:Fa]h) (::)\n";
    let score = read(ALT.as_bytes(), Format::Gabc).expect("GABC import");
    let Some(scoretk::model::Extension::Gregorian(payload)) = &score.extensions.format else {
        panic!("the L6 payload is present");
    };
    assert_eq!(payload.syllables[0].above_text.as_deref(), Some("Fa"));
    assert_eq!(score.notes.len(), 2, "[alt:Fa] is not a note");
    let xml = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert!(
        xml.contains("<direction placement=\"above\">") && xml.contains("<words>Fa</words>"),
        "a third-party reader sees the text: {xml}"
    );
    let gabc = String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC"))
        .expect("UTF-8 GABC");
    assert!(gabc.contains("[alt:Fa]"), "{gabc}");
}

/// `Gabc.tex:36-58,452-700` : pas de hampe, une liure par neume.
#[test]
fn gabc_notes_are_stemless_and_neumes_are_slurred() {
    const NEUMES: &str = "name:t;\n%%\n(c4) A(gh)ve(ih) (::)\n";
    let score = read(NEUMES.as_bytes(), Format::Gabc).expect("GABC import");
    let xml = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    assert_eq!(
        xml.matches("<stem>none</stem>").count(),
        4,
        "every sung pitch letter is stemless: {xml}"
    );
    assert_eq!(
        xml.matches("<slur number=\"1\" type=\"start\"/>").count(),
        2,
        "one slur start per neume: {xml}"
    );
    assert_eq!(
        xml.matches("<slur number=\"1\" type=\"stop\"/>").count(),
        2,
        "one slur stop per neume: {xml}"
    );
}

/// `Gabc.tex:241-245,314-317` : `Ma-rí-a` est un mot, pas trois syllabes
/// isolées ; MusicXML porte begin/middle/end.
#[test]
fn gabc_word_syllabics_reach_musicxml() {
    const WORDS: &str = "name:t;\n%%\n(c4) Ma(g)rí(h)a(i) sol(g) (::)\n";
    let score = read(WORDS.as_bytes(), Format::Gabc).expect("GABC import");
    let xml = String::from_utf8(
        write(&score, Format::MusicXml, &Options::default()).expect("MusicXML export"),
    )
    .expect("UTF-8 MusicXML");
    for value in ["begin", "middle", "end", "single"] {
        assert!(
            xml.contains(&format!("<syllabic>{value}</syllabic>")),
            "missing syllabic {value}: {xml}"
        );
    }
}

/// `Gabc.tex:742-797` : punctum mora × 2 ; l'épisème horizontal est un tenuto
/// noté sans point et joué ×1,5. MIDI et timeline gardent les mêmes durées.
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
    assert_eq!(ticks, [959, 719, 1439], "half, quarter ×1,5, half ×1,5");
    let xml = String::from_utf8(playback.musicxml).expect("UTF-8 MusicXML");
    assert!(xml.contains("<type>half</type>"), "{xml}");
    assert!(xml.contains("<type>quarter</type>"), "{xml}");
    assert_eq!(
        xml.matches("<dot/>").count(),
        0,
        "an episema is a tenuto, not a dot: {xml}"
    );
    assert_eq!(
        xml.matches("<tenuto/>").count(),
        2,
        "both episemata are tenutos: {xml}"
    );
    // Divisions = 2 : une noire vaut 2, le tenuto 3, la blanche 4 et
    // l'épisème sur la blanche 6.
    assert!(
        xml.contains("<duration>3</duration>"),
        "the tenuto note is played 1.5 quarters: {xml}"
    );
    assert!(
        xml.contains("<duration>6</duration>"),
        "mora + episema is played 1.5 halves: {xml}"
    );
}

/// Le tenuto revient à l'épisème `_` sans point ni autre glyphe, même quand
/// le MusicXML vient d'un tiers (aucun champ privé `gabc:`).
#[test]
fn musicxml_tenuto_returns_as_a_gabc_episema() {
    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<score-partwise version="4.0">
  <work><work-title>Tenuto</work-title></work>
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes>
      <divisions>2</divisions>
      <clef><sign>C</sign><line>4</line></clef>
      <staff-details><staff-lines>4</staff-lines></staff-details>
    </attributes>
    <note><pitch><step>G</step><octave>3</octave></pitch><duration>3</duration>
      <type>quarter</type>
      <notations><articulations><tenuto/></articulations></notations>
      <lyric number="1"><syllabic>single</syllabic><syllable>Do</syllable></lyric></note>
  </measure></part>
</score-partwise>"#;
    let score = read(XML.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let gabc = String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC"))
        .expect("UTF-8 GABC");
    assert!(
        gabc.contains("g_"),
        "le tenuto doit revenir en `_` : {gabc}"
    );
    assert!(
        !gabc.contains("g."),
        "aucun point de prolongation ne doit apparaître : {gabc}"
    );
    let again = read(gabc.as_bytes(), Format::Gabc).expect("relecture GABC");
    let Some(Extension::Gregorian(payload)) = &again.extensions.format else {
        panic!("charge grégorienne reconstruite");
    };
    assert!(
        payload
            .syllables
            .iter()
            .flat_map(|syllable| &syllable.notes)
            .flat_map(|note| &note.signs)
            .any(|sign| sign.kind == scoretk::model::SignKind::HorizontalEpisema),
        "le signe `_` doit survivre : {gabc}"
    );
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

/// Nombre de `<breath-mark>` par `<note>` dans l'export MusicXML.
fn breath_marks_per_note(xml: &str) -> Vec<usize> {
    xml.split("<note>")
        .skip(1)
        .map(|chunk| {
            chunk
                .split("</note>")
                .next()
                .unwrap_or("")
                .matches("<breath-mark")
                .count()
        })
        .collect()
}

fn gabc_modern_xml(source: &str) -> String {
    let score = read(source.as_bytes(), Format::Gabc).expect("GABC import");
    let options = Options {
        gabc_modern_clef: true,
        ..Options::default()
    };
    String::from_utf8(write(&score, Format::MusicXml, &options).expect("MusicXML")).expect("UTF-8")
}

/// Un punctum mora n'est pas une respiration, et la divisio finalis `::` est
/// une barre finale (`gabctk.py:935-936` : `||` ; `Gabc.tex:742-755,815-817`).
/// Aucune virgule ne doit donc apparaître sur `d(h._)` ni sur la dernière
/// note, malgré la durée jouée plus longue du mora.
#[test]
fn gabc_mora_and_finalis_do_not_emit_breath_marks() {
    const SOURCE: &str = "name:t;\n%%\n(c4) a(g) b(h_) c(g) d(h._) e(g) (::)\n";
    let xml = gabc_modern_xml(SOURCE);
    assert_eq!(breath_marks_per_note(&xml), [0, 0, 0, 0, 0], "{xml}");
    assert!(xml.contains("<bar-style>light-heavy</bar-style>"), "{xml}");
    // L'aller-retour GABC reste intact : la respiration n'est qu'une
    // projection moderne, aucun signe n'est réécrit dans le payload L6.
    let score = read(SOURCE.as_bytes(), Format::Gabc).expect("import");
    let again = String::from_utf8(write(&score, Format::Gabc, &Options::default()).expect("GABC"))
        .expect("UTF-8");
    let reread = read(again.as_bytes(), Format::Gabc).expect("second import");
    assert_eq!(reread.notes.len(), score.notes.len());
}

/// Un groupe de barres sans note `(,)` juste après la clé n'a aucune note
/// chantée précédente sur qui porter la respiration : `<breath-mark>` se
/// dessine après sa note, donc le reporter sur `do` le placerait après le mot
/// et de l'autre côté de la divisio. La finalis n'en ajoute aucune sur `fa`.
#[test]
fn gabc_note_less_bar_without_previous_sung_note_has_no_breath() {
    const SOURCE: &str = "name:t;\n%%\n(c4) (,) do(h_) re(g) mi(h._) fa(g) (::)\n";
    let xml = gabc_modern_xml(SOURCE);
    assert_eq!(breath_marks_per_note(&xml), [0, 0, 0, 0], "{xml}");
    assert!(xml.contains("<bar-style>light-heavy</bar-style>"), "{xml}");
}

/// Un `(,)` sans note propre remonte la respiration sur la dernière note
/// chantée *avant* la barre (`x`), pas sur la première après (`do`) : une
/// virgule MusicXML se dessine après la note qui la porte. Une seule virgule
/// par note, et aucune avant la finalis `::`.
#[test]
fn gabc_note_less_bar_breath_goes_backward_to_last_sung_note() {
    const SOURCE: &str = "name:t;\n%%\n(c4) x(g) (,) do(h_) re(g) (::)\n";
    let xml = gabc_modern_xml(SOURCE);
    assert_eq!(breath_marks_per_note(&xml), [1, 0, 0], "{xml}");
    assert!(xml.contains("<bar-style>light-heavy</bar-style>"), "{xml}");
}

/// Règle positive (`gabctk.py:930-936` : `,` → `!shortphrase![|]`,
/// `;` → `!mediumphrase![|]`) : une divisio minima porte exactement une
/// respiration, sur sa dernière note chantée, et la finalis reste sans.
#[test]
fn gabc_minima_breath_is_written_once_on_its_last_sung_note() {
    const SOURCE: &str = "name:t;\n%%\n(c4) sol(g) la(h._,) si(i) (::)\n";
    let xml = gabc_modern_xml(SOURCE);
    assert_eq!(breath_marks_per_note(&xml), [0, 1, 0], "{xml}");
}
