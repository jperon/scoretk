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

//! Ordre des agréments et des décorations à l'écriture (§ 4.20 de la norme
//! ABC 2.1) : « the order of abc constructs for a note is <grace notes>,
//! <chord symbols>, <annotations>/<decorations>, <accidentals>, <note>, … ».
//!
//! Le groupe d'agréments `{}` doit donc précéder les décorations (`.`, `!…!`).
//! Sinon `abc2midi` refuse la note (« Malformed note : expecting a-g or
//! A-G »), comme sur `Vingt_danses_enfantines_pour_piano(1).mxl`. Le comportement
//! de `xml2abc.py` est le même : `mkNote` (ligne 1217) ouvre le groupe `{}`
//! avant que `doDirection` (lignes 1040-1054) n'ajoute les décorations à
//! `note.before`, et `doMaat` (ligne 656) les écrit dans cet ordre.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

/// Deux portées : agrément avec acciaccatura et staccato, musique ordinaire
/// (accord, silence, liaison) dans la même voix et voix témoin sans agrément.
fn partition() -> String {
    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\">\
<work><work-title>Essai</work-title></work>\
<part-list>\
<score-part id=\"P1\"><part-name>Violon</part-name></score-part>\
<score-part id=\"P2\"><part-name>Alto</part-name></score-part>\
</part-list>\
<part id=\"P1\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<note><grace slash=\"yes\"/><pitch><step>B</step><octave>4</octave></pitch><type>eighth</type></note>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><articulations><staccato /></articulations></notations></note>\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><rest /><duration>2</duration><type>quarter</type></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><articulations><staccato /></articulations></notations><tie type=\"start\" /></note>\
<note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<tie type=\"stop\" /></note>\
<note><pitch><step>B</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
</measure>\
</part>\
<part id=\"P2\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><type>half</type>\
<notations><articulations><staccato /></articulations></notations></note>\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>F</step><octave>4</octave></pitch><duration>8</duration><type>whole</type></note>\
</measure>\
</part>\
</score-partwise>"
        .to_owned()
}

fn ecrire() -> String {
    let source = partition();
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    String::from_utf8(octets).expect("ABC UTF-8")
}

/// Lignes de musique d'une voix, sans l'en-tête `V:`.
fn voix(abc: &str, numero: u8) -> Vec<String> {
    let mut actif = false;
    abc.lines()
        .filter_map(|ligne| {
            if ligne.starts_with(&format!("V:{numero} ")) {
                actif = true;
                return None;
            }
            if ligne.starts_with("V:") {
                actif = false;
                return None;
            }
            actif.then(|| ligne.to_owned())
        })
        .collect()
}

#[test]
fn agrement_avant_les_decorations() {
    let abc = ecrire();
    let v1 = voix(&abc, 1);
    assert_eq!(v1.len(), 2, "deux mesures écrites : {v1:?}");
    // § 4.20 : le groupe d'agréments s'écrit avant le point de staccato ;
    // `./{B}` est une note mal formée pour abc2midi, `{/B}.C2` est correct.
    assert_eq!(v1[0], "{/B}.C2E2G2z2 |", "ordre des agréments : {v1:?}");
    // Musique ordinaire de la même voix : durées, accord de deux notes, tie.
    assert_eq!(v1[1], ".A2-A2B4 |]", "mesure ordinaire : {v1:?}");
    // Cas « non concerné » : sans agrément, la décoration reste en tête.
    let v2 = voix(&abc, 2);
    assert_eq!(v2, [".D4E4 |", "F8 |]"], "voix témoin : {v2:?}");
}
