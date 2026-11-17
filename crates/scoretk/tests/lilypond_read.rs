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
