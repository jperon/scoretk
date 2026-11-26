// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This file is original: tests of the implicit `nabc-lines` count of a
// headerless GABC body (raw GregoBase export), written in 2026.
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

//! A raw GregoBase body carries its `|` cuts but not the `nabc-lines` header
//! that GregoBase prepends before rendering (`gregobase/chant_img.php:48`,
//! `GregorioNabcRef.tex:153-170`). See `docs/agents/gabc.md` §17.

use scoretk_core::model::{Extension, Gregorian};
use scoretk_gabc_import::read_gabc;

/// One cut per group: the Gregorian alternating pattern of the reference.
const ALTERNATING: &str = "(f3) AL(ef~|ta>)le(fg|pe)lú(ef~|ta>)ia.(f.|ta-) (,)\n";
/// Seven cuts in one group: the value inferred must be the maximum, not 1.
const MANY_CUTS: &str = "(f3) (ii//|bv-|gh!ivHG//|vi-hhppu2su1sut1|fhg/|to|eef.|pt) (,)\n";

fn payload(text: &str) -> Gregorian {
    let score = read_gabc(text).expect("a GABC body");
    let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
        panic!("the L6 payload is present");
    };
    gregorian.clone()
}

/// Source offsets address different files (a headed file has a longer
/// prefix); headers are absent in the raw body. Compare the musical payload.
fn comparable(mut gregorian: Gregorian) -> Gregorian {
    gregorian.headers.clear();
    for syllable in &mut gregorian.syllables {
        for note in &mut syllable.notes {
            note.source_offset = None;
            note.source_end = None;
        }
    }
    gregorian
}

fn snippet_count(gregorian: &Gregorian) -> usize {
    gregorian
        .syllables
        .iter()
        .map(|syllable| syllable.nabc.len())
        .sum()
}

fn note_count(gregorian: &Gregorian) -> usize {
    gregorian
        .syllables
        .iter()
        .map(|syllable| syllable.notes.len())
        .sum()
}

#[test]
fn a_cut_in_a_headerless_body_is_read_like_an_explicit_nabc_lines_header() {
    let raw = payload(ALTERNATING);
    let headed = payload(&format!("name: Test;\nnabc-lines: 1;\n%%\n{ALTERNATING}"));
    assert_eq!(comparable(raw.clone()), comparable(headed));
    assert_eq!(snippet_count(&raw), 4);
    // `ef~` is two sung letters (the liquescent's letter is sung too),
    // `fg` is too; the manuscript text after each cut adds no note.
    assert_eq!(note_count(&raw), 7);
    let score = read_gabc(ALTERNATING).expect("derived view");
    assert_eq!(score.notes.len(), 7);
}

#[test]
fn the_inferred_count_is_the_maximum_per_group() {
    let raw = payload(MANY_CUTS);
    let headed = payload(&format!("name: Test;\nnabc-lines: 7;\n%%\n{MANY_CUTS}"));
    assert_eq!(comparable(raw.clone()), comparable(headed));
    assert_eq!(snippet_count(&raw), 7);
    // Only the square snippet before the first cut is sung.
    assert_eq!(note_count(&raw), 2);
}

#[test]
fn an_explicit_header_always_wins_even_when_it_is_zero() {
    let raw = payload(MANY_CUTS);
    let zero = payload(&format!("name: Test;\nnabc-lines: 0;\n%%\n{MANY_CUTS}"));
    let one = payload(&format!("name: Test;\nnabc-lines: 1;\n%%\n{MANY_CUTS}"));
    // With 0 (or 1) the state cycles modulo 2: the 2nd, 4th and 6th
    // segments after cuts fall back to square notation and are sung.
    assert!(note_count(&zero) > note_count(&raw));
    assert_eq!(comparable(zero), comparable(one));
}

#[test]
fn pipes_in_lyrics_comments_or_verbatim_tex_do_not_create_a_voice() {
    let body = "(c4) Ky|ri(f) e(g)lé(h) % a | comment with a cut\n <v>a|b</v> e(f) (::)\n";
    let gregorian = payload(body);
    assert_eq!(snippet_count(&gregorian), 0);
    assert_eq!(note_count(&gregorian), 4);
    assert_eq!(read_gabc(body).expect("derived view").notes.len(), 4);

    let headed = "% a | comment in the header\nname: Test;\n%%\n(c4) Ky(f) e(g) (::)\n";
    assert_eq!(snippet_count(&payload(headed)), 0);
}
