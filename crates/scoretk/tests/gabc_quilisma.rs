// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : tests de la projection moderne du quilisma
// grégorien (`docs/agents/gabc.md` §7), d'après la convention gabctk
// (`gabctk.py:35,45,932-933,1051-1095`), décision de l'utilisateur du
// 2026-11-25.
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

//! Un quilisma `w` est un ornement dans la projection moderne : il doit
//! apparaître dans les trois exports evenly lisibles (MusicXML, LilyPond, ABC)
//! et revenir à `w` après un aller-retour par du MusicXML standard.

use scoretk::model::{Extension, SignKind};
use scoretk::{read, write, Format, Options};

const GABC: &str = "name: Quilisma;\n%%\n(c3)De(ghiw)us(h;)\n";

#[test]
fn quilisma_est_un_ornement_dans_les_trois_exports() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("lecture GABC");
    let xml = String::from_utf8(write(&score, Format::MusicXml, &Options::default()).unwrap())
        .expect("MusicXML UTF-8");
    assert!(
        xml.contains("<inverted-mordent"),
        "le quilisma doit traverser MusicXML : {xml}"
    );
    let abc = String::from_utf8(write(&score, Format::Abc, &Options::default()).unwrap())
        .expect("ABC UTF-8");
    assert!(abc.contains("!uppermordent!"), "ABC : {abc}");
    let ly = String::from_utf8(write(&score, Format::Ly, &Options::default()).unwrap())
        .expect("LilyPond UTF-8");
    assert!(ly.contains("\\prall"), "LilyPond : {ly}");
}

/// MusicXML écrit à la main, sans aucun champ privé `gabc:` : c'est le seul
/// cas où un consommateur tiers peut reconnaître le quilisma.
const XML_STANDARD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<score-partwise version="4.0">
  <work><work-title>Quilisma</work-title></work>
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes>
      <divisions>1</divisions>
      <clef><sign>C</sign><line>3</line></clef>
      <staff-details><staff-lines>4</staff-lines></staff-details>
    </attributes>
    <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration>
      <type>quarter</type><lyric number="1"><syllabic>begin</syllabic><syllable>De</syllable></lyric></note>
    <note><pitch><step>B</step><octave>4</octave></pitch><duration>1</duration>
      <type>quarter</type>
      <notations><ornaments><inverted-mordent/></ornaments></notations>
      <lyric number="1"><syllabic>end</syllabic><syllable>us</syllable></lyric></note>
  </measure></part>
</score-partwise>"#;

#[test]
fn inverted_mordent_relu_depuis_musicxml_standard_devient_quilisma() {
    let score = read(XML_STANDARD.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let gabc = String::from_utf8(write(&score, Format::Gabc, &Options::default()).unwrap())
        .expect("GABC UTF-8");
    assert!(gabc.contains('w'), "le quilisma doit être réécrit : {gabc}");
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
            .any(|sign| sign.kind == SignKind::Quilisma),
        "le signe `w` doit survivre : {gabc}"
    );
}
