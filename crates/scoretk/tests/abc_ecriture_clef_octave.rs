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

//! Écriture ABC des clefs à décalage d'octave.
//!
//! Port de `xml2abc.py::doAttr` (lignes 1329-1331 : `clefOct[n] = -octave-change`)
//! et `mkNote` (ligne 1154 : `oct += clefOct[...]`) : `abc2midi` ne tient pas
//! compte du `-8` d'une clef `treble-8`, donc xml2abc décale les hauteurs
//! écrites d'autant d'octaves que nécessaire pour que la lecture soit juste.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

fn partition(octave: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\">\
<work><work-title>Essai</work-title></work>\
<part-list>\
<score-part id=\"P1\"><part-name>Ténor</part-name></score-part>\
<score-part id=\"P2\"><part-name>Soprano</part-name></score-part>\
</part-list>\
<part id=\"P1\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line>{octave}</clef></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type><tie type=\"start\"/></note>\
<note><rest/><duration>2</duration><type>quarter</type></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration><type>half</type><tie type=\"stop\"/></note>\
<note><rest/><duration>4</duration><type>half</type></note>\
</measure>\
</part>\
<part id=\"P2\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><rest measure=\"yes\"/><duration>8</duration></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><type>whole</type></note>\
</measure>\
</part>\
</score-partwise>"
    )
}

fn ecrire(octave: &str) -> String {
    let source = partition(octave);
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    String::from_utf8(octets).expect("ABC UTF-8")
}

#[test]
fn clef_treble_8_decale_les_hauteurs_ecrites() {
    let abc = ecrire("<clef-octave-change>-1</clef-octave-change>");
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(v1.contains("clef=treble-8"), "clef de la voix 1 : {v1}");
    let v2 = abc.lines().find(|l| l.starts_with("V:2 ")).expect("V:2");
    assert!(v2.contains("clef=treble"), "clef de la voix 2 : {v2}");
    // Musique ordinaire conservée dans la voix décalée : accord, silence,
    // liaison, deux mesures et barre finale.
    let music: Vec<&str> = abc.lines().skip_while(|l| !l.starts_with("V:1 ")).collect();
    let notes = music.iter().find(|l| l.contains('[')).expect("accord");
    assert!(notes.contains("c"), "C4 doit être écrit c' (C5) : {notes}");
    assert!(notes.contains('['), "accord : {notes}");
    assert!(notes.contains('z'), "silence : {notes}");
    assert!(notes.contains('-'), "liaison : {notes}");
    assert_eq!(abc.matches("|]").count(), 2, "deux portées");
    // Cas « non concerné » : la voix sans clef décalée garde ses hauteurs.
    let normal: Vec<&str> = abc.lines().skip_while(|l| !l.starts_with("V:2 ")).collect();
    assert!(
        normal.iter().any(|l| l.contains('C') && !l.contains('[')),
        "la voix 2 garde C4 : {normal:?}"
    );
}

#[test]
fn clef_treble_8_au_dessus_et_cas_sans_decalage() {
    let abc = ecrire("<clef-octave-change>1</clef-octave-change>");
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(v1.contains("clef=treble+8"), "clef haute : {v1}");
    let music: Vec<&str> = abc.lines().skip_while(|l| !l.starts_with("V:1 ")).collect();
    let notes = music.iter().find(|l| l.contains('[')).expect("accord");
    assert!(
        notes.contains("C,,") || notes.contains("C,"),
        "C4 doit être écrit plus bas : {notes}"
    );
    // Aucun décalage : la sortie est identique à celle d'une clef normale.
    let abc = ecrire("");
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(!v1.contains("treble-8") && !v1.contains("treble+8"), "{v1}");
}
