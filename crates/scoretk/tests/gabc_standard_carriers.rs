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

//! Facade checks for chant carriers that are in the destination **specification**:
//! XSD 4.0 `other-notation/@smufl` and LilyPond's ancient-notation contexts.

use scoretk::{read, read_abc_with_dialect, write, Format, Options};

const GABC: &str =
    "name: Veni Sancte Spiritus;\nlanguage: latin;\nmode: 2;\n%%\n(c3)\nVE(g.)ni(h') (,)\n";

/// A standard MusicXML document: no `gabc:` element at all. The SMuFL chant
/// glyph, the clef and a `miscellaneous-field` are the only carriers.
const XML: &str = r#"<score-partwise version="4.0">
  <identification><miscellaneous>
    <miscellaneous-field name="office-part">Communio</miscellaneous-field>
  </miscellaneous></identification>
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes>
      <clef><sign>C</sign><line>3</line></clef>
      <staff-details><staff-lines>4</staff-lines></staff-details>
    </attributes>
    <note><pitch><step>G</step><octave>3</octave></pitch>
      <duration>1</duration><type>quarter</type>
      <notations><other-notation type="single" smufl="chantQuilisma"/></notations>
      <lyric number="1"><syllabic>single</syllabic><text>Ve</text></lyric>
    </note>
    <barline location="right"><bar-style>light-heavy</bar-style></barline>
  </measure></part>
</score-partwise>"#;

#[test]
fn smufl_chant_glyph_is_a_standard_musicxml_carrier() {
    let score = read(XML.as_bytes(), Format::MusicXml).expect("MusicXML import");
    let gabc = write(&score, Format::Gabc, &Options::default()).expect("GABC export");
    let text = String::from_utf8(gabc).expect("UTF-8");
    assert!(text.contains("w"), "quilisma lost: {text}");
    assert!(
        text.contains("office-part: Communio"),
        "header lost: {text}"
    );
    assert!(
        text.contains(" :: ") || text.contains("(::)"),
        "final bar lost: {text}"
    );
    assert!(!text.contains("gabc:"), "private field invented: {text}");
}

#[test]
fn gregorian_abc_carriers_are_readable_and_additive() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let default = write(&score, Format::Abc, &Options::default()).expect("default .abc");
    let default = String::from_utf8(default).expect("UTF-8");
    assert!(
        !default.contains("I:chant=gregorian"),
        "default bytes changed"
    );

    let options = Options {
        abc_gregorian: true,
        ..Options::default()
    };
    let abc = write(&score, Format::Abc, &options).expect("Gregorian .abc");
    let abc = String::from_utf8(abc).expect("UTF-8");
    assert!(abc.contains("I:chant=gregorian"), "{abc}");
    assert!(abc.contains("I:clef=c3"), "{abc}");
    assert!(abc.contains("U: H = !fermata!"), "{abc}");
    assert!(abc.contains("\"^text:"), "{abc}");
    assert!(abc.contains("N:language=latin"), "{abc}");
    assert!(!abc.contains("gabc"), "no private prefix: {abc}");
    // Aucun code hexadécimal dans une décoration ou une annotation.
    assert!(!abc.contains("!gG"), "{abc}");

    let again = read_abc_with_dialect(abc.as_bytes(), true).expect("Gregorian .abc import");
    let gabc = write(&again, Format::Gabc, &Options::default()).expect("GABC export");
    let text = String::from_utf8(gabc).expect("UTF-8");
    assert!(text.contains("name: Veni Sancte Spiritus"), "{text}");
    assert!(text.contains("(c3)"), "clef lost: {text}");
    assert!(text.contains("language: latin"), "header lost: {text}");
    assert!(text.contains("VE("), "syllable text lost: {text}");
}

#[test]
fn gregorian_lilypond_option_is_additive_and_round_trips() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let default = write(&score, Format::Ly, &Options::default()).expect("default .ly");
    let default = String::from_utf8(default).expect("UTF-8");
    assert!(default.contains("\\fixed"), "default dialect changed");
    assert!(!default.contains("VaticanaVoice"));

    let options = Options {
        ly_gregorian: true,
        ..Options::default()
    };
    let ly = write(&score, Format::Ly, &options).expect("Gregorian .ly");
    let ly = String::from_utf8(ly).expect("UTF-8");
    assert!(ly.contains("\\new VaticanaVoice"), "{ly}");
    assert!(ly.contains("\\clef \"vaticana-do3\""), "{ly}");
    assert!(ly.contains("\\divisioMinima"), "{ly}");
    assert!(ly.contains("Custos_engraver"), "{ly}");

    let again = read(ly.as_bytes(), Format::Ly).expect("Gregorian .ly import");
    let gabc = write(&again, Format::Gabc, &Options::default()).expect("GABC export");
    let text = String::from_utf8(gabc).expect("UTF-8");
    assert!(text.contains("name: Veni Sancte Spiritus"), "{text}");
    assert!(text.contains("(c3)"), "clef lost: {text}");
}
