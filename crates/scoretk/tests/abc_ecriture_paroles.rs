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

//! Lignes `w:` d'une voix qui a des paroles.
//!
//! Port de `xml2abc.py::writePart` (lignes 341-355) : pour chaque couplet
//! d'une voix, une ligne `w:` suit **chaque** ligne de musique qui contient
//! des notes ; les mesures sans paroles reçoivent des `*` de remplissage.
//! Sinon `abc2midi` avertit « Line of music without lyrics » sur chaque ligne
//! de musique d'une voix chantée.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

fn partition() -> String {
    let notes = |step: &str, octave: &str, lyric: Option<&str>| {
        let lyric = lyric
            .map(|l| {
                format!("<lyric number=\"1\"><syllabic>single</syllabic><text>{l}</text></lyric>")
            })
            .unwrap_or_default();
        format!(
            "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch>\
<duration>2</duration><type>quarter</type>{lyric}</note>"
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\">\
<work><work-title>Chant</work-title></work>\
<part-list><score-part id=\"P1\"><part-name>Voix</part-name></score-part></part-list>\
<part id=\"P1\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
{}{}{}{}\
</measure>\
<measure number=\"2\">{}{}{}{}</measure>\
<measure number=\"3\">{}{}{}{}</measure>\
<measure number=\"4\"><note><rest measure=\"yes\"/><duration>8</duration></note></measure>\
</part></score-partwise>",
        notes("C", "4", Some("la")),
        notes("D", "4", Some("la")),
        notes("E", "4", Some("la")),
        notes("F", "4", Some("la")),
        notes("G", "4", None),
        notes("A", "4", None),
        notes("B", "4", None),
        notes("C", "5", None),
        notes("D", "5", Some("mi")),
        notes("E", "5", Some("mi")),
        notes("F", "5", Some("mi")),
        notes("G", "5", Some("mi")),
    )
}

fn ecrire() -> String {
    let source = partition();
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    String::from_utf8(octets).expect("ABC UTF-8")
}

#[test]
fn chaque_ligne_avec_notes_a_sa_ligne_de_paroles() {
    let abc = ecrire();
    let debut = abc
        .lines()
        .position(|l| l.starts_with("C2D2E2F2"))
        .expect("première mesure");
    let lignes: Vec<&str> = abc.lines().skip(debut).collect();
    // m1 paroles, m2 sans paroles, m3 paroles, m4 silence plein.
    assert_eq!(lignes[0], "C2D2E2F2 |");
    assert_eq!(lignes[1], "w: la la la la");
    assert_eq!(lignes[2], "G2A2B2c2 |");
    assert_eq!(
        lignes[3], "w: * * * *",
        "la mesure chantée sans paroles doit porter une ligne w: : {abc}"
    );
    assert_eq!(lignes[4], "d2e2f2g2 |");
    assert_eq!(lignes[5], "w: mi mi mi mi");
    // Cas « non concerné » : une mesure sans notes ne reçoit pas de `w:`.
    assert_eq!(lignes[6], "z8 |]"); // explicit z fill (xml2abc.py::doMaat)
    assert_eq!(lignes.len(), 7, "aucune ligne supplémentaire : {abc}");
}
