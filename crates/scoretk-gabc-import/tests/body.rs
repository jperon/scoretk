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

//! Headerless GABC bodies (raw GregoBase exports), accepted as a tolerance
//! beyond Gregorio; see `docs/agents/gabc.md` §15.

use scoretk_core::model::Extension;
use scoretk_gabc_import::read_gabc;

/// The exact shape of a raw GregoBase export, as rendered by exsurge.
const BODY: &str = "(c4) Ky(f)ri(g)e(h) e(g)lé(f)i(e)son.(f) (::)\n";

fn payload(text: &str) -> scoretk_core::model::Gregorian {
    let score = read_gabc(text).expect("a GABC body");
    let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
        panic!("the L6 payload is present");
    };
    assert!(gregorian.derived_modern_view);
    gregorian.clone()
}

#[test]
fn a_body_only_file_is_read_with_an_empty_header() {
    let gregorian = payload(BODY);
    assert!(gregorian.headers.is_empty());
    assert_eq!(gregorian.staff_lines, 4); // default: `Gabc.tex:128,434`
    assert_eq!(gregorian.clefs, ["c4"]);
    let letters: String = gregorian
        .syllables
        .iter()
        .flat_map(|syllable| syllable.notes.iter())
        .map(|note| note.letter)
        .collect();
    assert_eq!(letters, "fghgfef");
    assert_eq!(
        gregorian.syllables.last().expect("a bar group").bars.len(),
        1
    );
    let score = read_gabc(BODY).expect("a GABC body");
    assert_eq!(score.notes.len(), 7);
}

#[test]
fn body_note_offsets_address_the_real_source_not_a_synthetic_marker() {
    let gregorian = payload(BODY);
    let first = gregorian.syllables[0].notes[0].clone();
    assert_eq!(first.letter, 'f');
    assert_eq!(first.source_offset, Some(8)); // `(c4) Ky(`
    assert_eq!(first.source_end, Some(9));
}

#[test]
fn leading_comments_and_a_syllable_first_body_are_accepted() {
    let commented = "\n % raw GregoBase export\n (c4) Ky(f)ri(g)e(h) (::)\n";
    let gregorian = payload(commented);
    assert_eq!(gregorian.clefs, ["c4"]);
    assert_eq!(gregorian.syllables.len(), 4);

    // No opening clef group: the first word is directly followed by a group.
    let no_clef = "Ky(f)ri(g)e(h) e(g)lé(f)i(e)son.(f) (::)\n";
    let gregorian = payload(no_clef);
    assert!(gregorian.clefs.is_empty());
    assert_eq!(read_gabc(no_clef).expect("score").notes.len(), 7);
}

#[test]
fn a_header_without_the_separator_is_still_an_error() {
    assert!(read_gabc("name: Veni;\n(c3) Ve(g)\n").is_err());
    assert!(read_gabc("%abc-2.1\nX:1\nT:Test\n").is_err());
}

#[test]
fn files_with_the_separator_keep_their_existing_offsets() {
    const HEADER: &str = "name: Veni;\n%%\n";
    let source = format!("{HEADER}{BODY}");
    let gregorian = payload(&source);
    let first = gregorian.syllables[0].notes[0].clone();
    // `%%` and its line end belong to the header for offset accounting.
    assert_eq!(first.source_offset, Some(HEADER.encode_utf16().count() + 8));
}
