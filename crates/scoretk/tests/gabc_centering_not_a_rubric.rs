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

//! Regression: vowel-centering braces are not a printed rubric.
//!
//! `Gabc.tex:314-324` makes `{}` a centering control: it is never printed.
//! `mi(h){n}us(h)` is the ordinary sung syllable *nus*, so no `<words>`
//! carrier may repeat it, colour it red or expose the raw GABC markup.

use scoretk::{read, write, Format, Options};

const GABC: &str = "name: Populus Sion;\nlanguage: latin;\nmode: 2;\n%%\n(c4)\n\
                    Dó(e.f!gwhhi)mi(h){n}us(h) vé(hi)ni(ig//ih)et(h.) (::)\n";

#[test]
fn centering_braces_never_become_a_words_carrier() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let options = Options {
        xml_full_support: true,
        ..Options::default()
    };
    let xml = write(&score, Format::MusicXml, &options).expect("MusicXML export");
    let xml = String::from_utf8(xml).expect("UTF-8");

    // The lyric keeps the sung word, with the invisible braces removed.
    assert!(xml.contains("<text>nus</text>"), "lyric lost: {xml}");
    // No duplicated, red, raw-markup <words> for the centering brace. The
    // private `gabc:syllable` field is gone from full-support output: a
    // standard-only consumer sees the sung syllable and nothing else.
    assert!(!xml.contains("<words"), "an unexpected <words>: {xml}");
    assert!(!xml.contains("gabc:"), "a private dump: {xml}");
    assert!(
        !xml.contains("#FF0000"),
        "syllable text coloured red: {xml}"
    );
}

#[test]
fn centering_is_an_honest_loss_after_full_support_round_trip() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let options = Options {
        xml_full_support: true,
        ..Options::default()
    };
    let xml = write(&score, Format::MusicXml, &options).expect("MusicXML export");
    let again = read(&xml, Format::MusicXml).expect("MusicXML import");
    let gabc = write(&again, Format::Gabc, &Options::default()).expect("GABC export");
    let gabc = String::from_utf8(gabc).expect("UTF-8");
    // MusicXML has no carrier for a GABC centering control: the sung syllable
    // survives in `<lyric>`, the invisible braces do not, and no hidden source
    // spelling is smuggled back in (`gabc.md` §29).
    assert!(gabc.contains("nus"), "sung syllable lost: {gabc}");
    assert!(!gabc.contains("{n}"), "centering smuggled back in: {gabc}");
    assert!(
        !gabc.contains("<words"),
        "no <words> is copied into GABC: {gabc}"
    );
}
