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

//! Lecture LilyPond branchée dans la façade (`feature = "lilypond-import"`).

#![cfg(feature = "lilypond-import")]

use scoretk::{read, write, Format, Options};

const SOURCE: &str = r#"\version "2.24.0"
\language "english"
\score { \new Staff { \fixed c' { \time 4/4 c'4 d'4 e'4 f'4 \bar "|." } } }
"#;

#[test]
fn lecture_lilypond_et_export_musicxml() {
    assert!(Format::Ly.supports_read());
    let score = read(SOURCE.as_bytes(), Format::Ly).expect("lecture LilyPond");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.notes.len(), 4);
    let xml = write(&score, Format::MusicXml, &Options::default()).expect("export");
    let xml = String::from_utf8(xml).expect("UTF-8");
    assert!(xml.contains("<score-partwise"));
    assert!(xml.contains("<step>C</step>"));
}

#[test]
fn source_illisible_est_une_erreur() {
    let error = read(b"\\version \"2.24.0\"\n", Format::Ly).expect_err("erreur attendue");
    assert!(matches!(error, scoretk::Error::LilyPond(_)));
}

/// Textes des syllabes de la première piste, dans l'ordre.
fn syllabes(score: &scoretk_core::model::Score) -> Vec<String> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(scoretk_core::model::Track::from_raw(0)))
        .flat_map(|chord_rest| chord_rest.lyrics.iter())
        .map(|lyrics| lyrics.text.clone())
        .collect()
}

#[test]
fn paroles_mot_a_espace_survit_a_l_aller_retour_lilypond() {
    // `Ah_!` est une syllabe « Ah ! » : elle doit rester une seule syllabe au
    // fil des conversions, et l'export `.ly` la protéger par des guillemets.
    const SOURCE: &str = r#"\version "2.24.0"
\language "english"
\score { << \new Voice = "v" { \fixed c' { c4 d e f } }
  \new Lyrics \lyricsto "v" { Ah_! qu'il fait bon } >> }
"#;
    let score = read(SOURCE.as_bytes(), Format::Ly).expect("lecture LilyPond");
    let xml = String::from_utf8(write(&score, Format::MusicXml, &Options::default()).expect("xml"))
        .expect("UTF-8");
    assert!(xml.contains("<text>Ah !</text>"), "{xml}");
    assert!(!xml.contains("<extend"));
    let ly = String::from_utf8(write(&score, Format::Ly, &Options::default()).expect("ly"))
        .expect("UTF-8");
    assert!(ly.contains("\"Ah !\""), "{ly}");
    let relu = read(ly.as_bytes(), Format::Ly).expect("relecture LilyPond");
    assert_eq!(syllabes(&relu), vec!["Ah !", "qu'il", "fait", "bon"]);
}

/// `midiInstrument` doit traverser la façade jusqu'aux deux exports : le
/// `<midi-program>` MusicXML est en numérotation GM (base un), l'évènement SMF
/// `Program Change` est en base zéro (`scm/midi.scm::midi-program`).
#[test]
fn midi_instrument_atteint_musicxml_et_midi() {
    const SOURCE: &str = r#"\version "2.24.0"
\score {
  <<
    \new Staff \with { midiInstrument = "violin" } { \fixed c' { c4 d e f } }
    \new Staff { \fixed c' { \set Staff.midiInstrument = "flute" g4 a b c' } }
  >>
}
"#;
    let score = read(SOURCE.as_bytes(), Format::Ly).expect("lecture LilyPond");
    let xml = String::from_utf8(write(&score, Format::MusicXml, &Options::default()).expect("xml"))
        .expect("UTF-8");
    assert!(xml.contains("<midi-program>41</midi-program>"), "{xml}");
    assert!(xml.contains("<midi-program>74</midi-program>"), "{xml}");
    let midi = write(&score, Format::Midi, &Options::default()).expect("midi");
    assert!(
        midi.windows(2).any(|bytes| bytes == [0xC0, 40]),
        "programme violon sur le canal 0"
    );
    assert!(
        midi.windows(2).any(|bytes| bytes == [0xC1, 73]),
        "programme flûte sur le canal 1"
    );
}
