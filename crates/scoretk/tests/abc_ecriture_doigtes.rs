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

//! Doigtés écrits dans une musique ordinaire (§ 4.14 de la norme ABC 2.1).
//!
//! Les noms de symboles ne peuvent pas contenir d'espace (§ 4.14) : MuseScore
//! stocke parfois le doigté avec des balises `<font …>`. L'écrivain ne doit
//! garder que le symbole de doigté lui-même, sinon `abc2midi` avertit
//! « instruction !<font! ignored » pour chaque note.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

/// `<fingering>` avec deux balises de mise en forme avant le chiffre, doigté
/// nu, puis balises seules (aucun symbole à écrire).
fn source() -> String {
    let markup =
        "&lt;font size='9'&gt;&lt;/font&gt; &lt;font face='Times New Roman'&gt;&lt;/font&gt; ";
    let note = |step: &str, octave: u8, fingering: &str, tie: &str| {
        format!(
            "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch>\
<duration>2</duration><type>quarter</type>{tie}\
<notations><technical><fingering>{fingering}</fingering></technical></notations></note>"
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\">\
<part-list><score-part id=\"P1\"><part-name>Chant</part-name></score-part>\
<score-part id=\"P2\"><part-name>Témoin</part-name></score-part></part-list>\
<part id=\"P1\">\
<measure number=\"1\"><attributes><divisions>2</divisions>\
<key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
{note_c}{note_e}{note_g}<note><rest/><duration>2</duration><type>quarter</type></note></measure>\
<measure number=\"2\">{chord}{note_d1}{note_d2}</measure></part>\
<part id=\"P2\">\
<measure number=\"1\"><attributes><divisions>2</divisions>\
<key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>F</sign><line>4</line></clef></attributes>\
<note><pitch><step>C</step><octave>3</octave></pitch><duration>8</duration><type>whole</type></note></measure>\
<measure number=\"2\"><note><pitch><step>G</step><octave>3</octave></pitch>\
<duration>8</duration><type>whole</type></note></measure></part></score-partwise>",
        note_c = note("C", 4, &format!("{markup}1"), ""),
        note_e = note("E", 4, "3", ""),
        note_g = note("G", 4, markup, ""),
        chord = "<note><voice>1</voice><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration>\
<type>quarter</type><notations><technical><fingering>5</fingering></technical></notations></note>\
<note><voice>1</voice><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><voice>1</voice><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>",
        note_d1 = note("D", 4, "2", "<tie type=\"start\"/>"),
        note_d2 = note("D", 4, "4", "<tie type=\"stop\"/>"),
    )
}

#[test]
fn doigtes_markup_ecrits_comme_symboles_valides() {
    let source = source();
    let score = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&score, Format::Abc, &Options::default()).expect("écriture ABC");
    let abc = String::from_utf8(octets).expect("ABC UTF-8");
    let debut = abc
        .lines()
        .position(|l| l.starts_with("V:1 "))
        .expect("en-tête V:1")
        + 1;
    let musique: Vec<&str> = abc
        .lines()
        .skip(debut)
        .take_while(|l| !l.starts_with("V:"))
        .filter(|l| !l.starts_with('w'))
        .collect();
    assert_eq!(
        musique,
        ["!1!C2!3!E2G2z2 |", "!5![CEG]2!2!D2-!4!D2x2 |]",],
        "musique de la voix chantée : {musique:?}"
    );
    // Aucun fragment de balise dans un nom de symbole (§ 4.14).
    assert!(
        !abc.contains("<font"),
        "balise MuseScore dans l'ABC : {abc}"
    );
    assert!(!abc.contains("!<"), "décoration invalide : {abc}");
    // Cas « non concerné » : la portée témoin ne reçoit aucun doigté.
    let temoin: Vec<&str> = abc.lines().skip_while(|l| !l.starts_with("V:2")).collect();
    assert!(
        temoin.iter().all(|l| !l.contains('!')),
        "voix témoin : {temoin:?}"
    );
}
