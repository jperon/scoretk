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
/// Seven cuts in one group: GregoBase declares one voice even there, so the
/// cuts alternate square and manuscript snippets (`.y:879,906`).
const MANY_CUTS: &str = "(f3) (ii//|bv-|gh!ivHG//|vi-hhppu2su1sut1|fhg/|to|eef.|pt) (,)\n";
/// GregoBase chant 18205 (`g00499`, "Deus tu convertens"), private SQL dump
/// `gregobase_online.sql`: thirteen cuts in the `nos,` group, no header.
const GREGOBASE_18205: &str =
    "(c4) nos,(hg|cl|h|st|jj|ds-hi|jvHG|ci|hh|ds-|/giHG|pesu2|hf/fe|cl!pi) (:)\n";

fn payload(text: &str) -> Gregorian {
    let score = read_gabc(text).expect("a GABC body");
    let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
        panic!("the L6 payload is present");
    };
    gregorian.clone()
}

/// Source offsets address different files (a headed file has a longer
/// prefix); headers are absent in the raw body. Compare everything except the
/// source-*layout* fields: where the explicit header ends, how its lines are
/// spelled, and the exact gap it leaves before the first group are not part of
/// the NABC reading. All musical fields (pitches, glyphs, bars, cuts, snippet
/// texts, spacings between syllables) remain compared.
fn comparable(mut gregorian: Gregorian) -> Gregorian {
    gregorian.headers.clear();
    gregorian.header_items.clear();
    gregorian.header_double_semicolons.clear();
    gregorian.notation_prefix = None;
    gregorian.opening_separator = None;
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

fn square_letters(gregorian: &Gregorian) -> String {
    gregorian
        .syllables
        .iter()
        .flat_map(|syllable| syllable.notes.iter())
        .map(|note| note.letter)
        .collect()
}

fn manuscript_texts(gregorian: &Gregorian) -> Vec<&str> {
    gregorian
        .syllables
        .iter()
        .flat_map(|syllable| syllable.nabc.iter())
        .map(|snippet| snippet.text.as_str())
        .collect()
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
fn seven_cuts_still_alternate_with_one_inferred_line() {
    let raw = payload(MANY_CUTS);
    let headed = payload(&format!("name: Test;\nnabc-lines: 1;\n%%\n{MANY_CUTS}"));
    assert_eq!(comparable(raw.clone()), comparable(headed));
    // State 0/1 alternates at every cut (`gabc-score-determination.y:879,906`):
    // segments 0, 2, 4, 6 are square, segments 1, 3, 5, 7 are manuscript.
    assert_eq!(square_letters(&raw), "iighiHGfhgeef");
    assert_eq!(
        manuscript_texts(&raw),
        ["bv-", "vi-hhppu2su1sut1", "to", "pt"]
    );
    assert_eq!(snippet_count(&raw), 4);
    assert_eq!(note_count(&raw), 13);
}

#[test]
fn an_explicit_header_always_wins_even_when_it_is_zero() {
    let raw = payload(MANY_CUTS);
    let zero = payload(&format!("name: Test;\nnabc-lines: 0;\n%%\n{MANY_CUTS}"));
    let one = payload(&format!("name: Test;\nnabc-lines: 1;\n%%\n{MANY_CUTS}"));
    let seven = payload(&format!("name: Test;\nnabc-lines: 7;\n%%\n{MANY_CUTS}"));
    // 0 (or 1) cycles modulo 2; an explicit 7 keeps every cut in NABC state:
    // only the two square notes before the first cut are sung then.
    assert_eq!(comparable(raw.clone()), comparable(one.clone()));
    assert_eq!(comparable(zero), comparable(one));
    assert_eq!(snippet_count(&seven), 7);
    assert_eq!(note_count(&seven), 2);
    assert_ne!(comparable(seven), comparable(raw));
}

#[test]
fn gregobase_18205_thirteen_cuts_alternate_with_one_inferred_line() {
    let raw = payload(GREGOBASE_18205);
    let headed = payload(&format!(
        "name: Test;\nnabc-lines: 1;\n%%\n{GREGOBASE_18205}"
    ));
    assert_eq!(comparable(raw.clone()), comparable(headed));
    // GregoBase renders raw bodies after prepending `nabc-lines: 1;`
    // (`specs/gabc/gregobase/chant_img.php:48`), and each cut cycles the
    // state modulo 2 (`gabc-score-determination.y:879,906`): the 0th, 2nd,
    // 4th, 6th, 8th, 10th and 12th segments are square (18 sung notes),
    // the seven others are the manuscript snippets.
    assert_eq!(square_letters(&raw), "hghjjjHGhhgiHGhffe");
    assert_eq!(
        manuscript_texts(&raw),
        ["cl", "st", "ds-hi", "ci", "ds-", "pesu2", "cl!pi"]
    );
    assert_eq!(snippet_count(&raw), 7);
    assert_eq!(note_count(&raw), 18);
    assert_eq!(
        read_gabc(GREGOBASE_18205)
            .expect("derived view")
            .notes
            .len(),
        18
    );
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
