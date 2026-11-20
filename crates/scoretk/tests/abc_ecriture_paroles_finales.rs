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

//! Paroles terminales réduites à des `*`.
//!
//! § 5.1 de la norme ABC 2.1 : « * is equivalent to a blank syllable » et
//! « if there are fewer syllables than available notes, the remaining notes
//! have no lyric (blank syllables) ». Une ligne `w: * * * *` est donc
//! nécessaire entre deux couplets (elle consomme les notes sans paroles du
//! couplet courant, sinon le couplet suivant se décale) et reste licite après
//! le dernier : elle déclare les syllabes blanches des notes restantes, comme
//! `xml2abc.py::writePart` (lignes 331-355) écrit un `w:` après chaque ligne
//! de musique chantée. Sans elle, `abc2midi` avertit « Line of music without
//! lyrics » sur une ligne qui porte pourtant des notes. Le silence final, lui,
//! n'est pas aligné (§ 5.1 : « syllables are not aligned on … rests ») et ne
//! réclame aucun `w:`.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

/// Une voix chantée : couplet, mesure sans paroles, couplet, mesure sans
/// paroles, couplet, mesure sans paroles en fin de morceau ; une portée témoin
/// sans paroles.
fn partition() -> String {
    let attrs = "<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>";
    let note = |step: &str, oct: u8, lyric: Option<&str>| {
        let lyric = lyric.map_or(String::new(), |text| {
            format!("<lyric number=\"1\"><syllabic>single</syllabic><text>{text}</text></lyric>")
        });
        format!(
            "<note><pitch><step>{step}</step><octave>{oct}</octave></pitch>\
<duration>2</duration><type>quarter</type>{lyric}</note>"
        )
    };
    let mesure = |num: usize, notes: &str, first: bool| {
        format!(
            "<measure number=\"{num}\">{}{notes}</measure>",
            if first { attrs } else { "" }
        )
    };
    let v1 = format!(
        "{}{}{}{}{}{}",
        mesure(
            1,
            &format!(
                "{}{}{}{}",
                note("C", 4, Some("do")),
                note("D", 4, Some("re")),
                note("E", 4, None),
                note("F", 4, None)
            ),
            true
        ),
        mesure(
            2,
            &format!(
                "{}{}{}{}",
                note("G", 4, None),
                note("A", 4, None),
                note("B", 4, None),
                note("c", 5, None)
            ),
            false
        ),
        mesure(
            3,
            &format!(
                "{}{}{}{}",
                note("d", 5, Some("mi")),
                note("c", 5, Some("fa")),
                note("B", 4, None),
                note("A", 4, None)
            ),
            false
        ),
        mesure(
            4,
            &format!(
                "{}{}{}{}",
                note("G", 4, None),
                note("F", 4, None),
                note("E", 4, None),
                note("D", 4, None)
            ),
            false
        ),
        mesure(
            5,
            &format!(
                "{}{}{}{}",
                note("C", 4, Some("sol")),
                note("D", 4, Some("la")),
                note("E", 4, None),
                note("F", 4, None)
            ),
            false
        ),
        mesure(
            6,
            &format!(
                "{}{}{}{}",
                note("G", 4, None),
                note("A", 4, None),
                note("B", 4, None),
                note("c", 5, None)
            ),
            false
        ),
    );
    let v2 = format!(
        "<measure number=\"1\">{attrs}<note><pitch><step>C</step><octave>3</octave></pitch>\
<duration>4</duration><type>half</type></note>\
<note><pitch><step>E</step><octave>3</octave></pitch><duration>4</duration><type>half</type></note></measure>\
<measure number=\"2\"><note><pitch><step>G</step><octave>3</octave></pitch>\
<duration>8</duration><type>whole</type></note></measure>\
<measure number=\"3\"><note><pitch><step>C</step><octave>3</octave></pitch>\
<duration>8</duration><type>whole</type></note></measure>\
<measure number=\"4\"><note><pitch><step>G</step><octave>3</octave></pitch>\
<duration>8</duration><type>whole</type></note></measure>\
<measure number=\"5\"><note><pitch><step>C</step><octave>3</octave></pitch>\
<duration>8</duration><type>whole</type></note></measure>\
<measure number=\"6\"><note><pitch><step>G</step><octave>3</octave></pitch>\
<duration>8</duration><type>whole</type></note></measure>"
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\"><work><work-title>Chant</work-title></work>\
<part-list><score-part id=\"P1\"><part-name>Voix</part-name></score-part>\
<score-part id=\"P2\"><part-name>Orgues</part-name></score-part></part-list>\
<part id=\"P1\">{v1}</part><part id=\"P2\">{v2}</part></score-partwise>"
    )
}

fn ecrire() -> String {
    let source = partition();
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    String::from_utf8(octets).expect("ABC UTF-8")
}

#[test]
fn w_etoiles_terminal_conserve_et_etoiles_intermediaires_conserve() {
    let abc = ecrire();
    let lignes: Vec<&str> = abc.lines().collect();
    let w: Vec<&str> = lignes
        .iter()
        .copied()
        .filter(|l| l.starts_with("w:"))
        .collect();
    // Une ligne par mesure chantée : deux servent d'alignement entre couplets,
    // trois portent des syllabes, la dernière ne déclare que des syllabes
    // blanches (§ 5.1) mais évite l'avertissement `abc2midi`.
    assert_eq!(
        w,
        [
            "w: do re * *",
            "w: * * * *",
            "w: mi fa * *",
            "w: * * * *",
            "w: sol la * *",
            "w: * * * *",
        ],
        "paroles écrites : {w:?}"
    );
    // Hauteurs et durées inchangées, six mesures dans la voix chantée.
    let debut = lignes
        .iter()
        .position(|l| l.starts_with("V:1 "))
        .expect("en-tête V:1")
        + 1;
    let music: Vec<&str> = lignes[debut..]
        .iter()
        .copied()
        .take_while(|l| !l.starts_with("V:"))
        .filter(|l| !l.starts_with("w:"))
        .collect();
    assert_eq!(
        music,
        [
            "C2D2E2F2 |",
            "G2A2B2c2 |",
            "c2c2B2A2 |",
            "G2F2E2D2 |",
            "C2D2E2F2 |",
            "G2A2B2c2 |]",
        ],
        "musique de la voix chantée : {music:?}"
    );
    // Toute ligne de musique portant des notes dans la voix chantée est
    // suivie d'un `w:` : c'est exactement ce que `abc2midi` exige pour ne pas
    // émettre « Line of music without lyrics » (§ 5.1 : syllabes blanches).
    let chant: Vec<&str> = lignes[debut..]
        .iter()
        .copied()
        .take_while(|l| !l.starts_with("V:"))
        .collect();
    for (i, ligne) in chant.iter().enumerate() {
        if ligne.is_empty() || ligne.starts_with("w:") {
            continue;
        }
        assert!(
            chant
                .get(i + 1)
                .is_some_and(|suivant| suivant.starts_with("w:")),
            "ligne chantée sans `w:` : {ligne:?}"
        );
    }
    // Cas « non concerné » : la portée sans paroles n'en reçoit aucune.
    let fin: Vec<&str> = lignes
        .iter()
        .copied()
        .skip_while(|l| !l.starts_with("V:2"))
        .collect();
    assert!(
        !fin.iter().any(|l| l.starts_with("w:")),
        "voix témoin : {fin:?}"
    );
}
