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

//! Écriture ABC d'instruments transpositeurs.
//!
//! Port de `xml2abc.py::doAttr` (lignes 1278, 1319-1332) : la transposition
//! chromatique, y compris l'`octave-change`, doit être écrite dans l'en-tête
//! `V:` pour qu'`abc2midi` joue la hauteur concert. Sans elle, les notes
//! écrites sont jouées à la hauteur notée et le MIDI diffère de la référence.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

/// Petite partition ordinaire : deux mesures, accord, silence, liaison, et
/// deux parties dont une seulement est transpositrice.
fn partition(transposition: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\">\
<work><work-title>Essai</work-title></work>\
<part-list>\
<score-part id=\"P1\"><part-name>Clarinette</part-name></score-part>\
<score-part id=\"P2\"><part-name>Violon</part-name></score-part>\
</part-list>\
<part id=\"P1\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef>{transposition}</attributes>\
<note><pitch><step>c</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><chord/><pitch><step>e</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><pitch><step>g</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type><tie type=\"start\"/></note>\
<note><rest/><duration>2</duration><type>quarter</type></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>g</step><octave>4</octave></pitch><duration>4</duration><type>half</type><tie type=\"stop\"/></note>\
<note><rest/><duration>4</duration><type>half</type></note>\
</measure>\
</part>\
<part id=\"P2\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<note><pitch><step>c</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><rest measure=\"yes\"/><duration>8</duration></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>c</step><octave>4</octave></pitch><duration>8</duration><type>whole</type></note>\
</measure>\
</part>\
</score-partwise>"
    )
}

fn ecrire(transposition: &str) -> String {
    let source = partition(transposition);
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    String::from_utf8(octets).expect("ABC UTF-8")
}

#[test]
fn transposition_octave_dans_les_declinaisons_de_voix() {
    let abc = ecrire(
        "<transpose><diatonic>0</diatonic><chromatic>0</chromatic>\
         <octave-change>-1</octave-change></transpose>",
    );
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(
        v1.contains("transpose=-12"),
        "octave-change doit être ajouté au chromatique : {v1}"
    );
    // La voix non transpositrice est le cas « non concerné » : inchangée.
    let v2 = abc.lines().find(|l| l.starts_with("V:2 ")).expect("V:2");
    assert!(
        !v2.contains("transpose="),
        "V:2 ne doit pas transposer : {v2}"
    );
    // Musique ordinaire : deux mesures, accord, silence, liaison conservés.
    assert!(abc.contains("|"), "barres absentes");
    assert!(abc.contains('['), "accord absent");
    assert!(abc.contains('z'), "silence absent");
    assert_eq!(
        abc.matches("|]").count(),
        2,
        "deux portées doivent se terminer"
    );
}

/// Une ottava n'est pas une transposition d'instrument : en ABC la hauteur
/// écrite est déjà décalée pour compenser le fait qu'`abc2midi` ignore l'ottava.
/// Ajouter `transpose=` dans ce cas rejouerait les notes une octave à côté.
#[test]
fn ottava_ne_devient_pas_une_transposition() {
    let source = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\">\
<part-list><score-part id=\"P1\"><part-name>Flûte</part-name></score-part></part-list>\
<part id=\"P1\"><measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<direction><direction-type><octave-shift type=\"down\"/></direction-type></direction>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><rest measure=\"yes\"/><duration>6</duration></note>\
</measure></part></score-partwise>";
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    let abc = String::from_utf8(octets).expect("ABC UTF-8");
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(
        !v1.contains("transpose="),
        "une ottava ne s'écrit pas comme une transposition : {v1}"
    );
    // La hauteur écrite est compensée d'une octave (C4 → C, ou c'), mais
    // n'est pas rejouée une seconde fois par un champ `transpose=`.
    let note = abc
        .lines()
        .find(|l| l.starts_with("C") || l.starts_with("c"))
        .expect("note");
    assert!(
        note.starts_with("C,") || note.starts_with('c'),
        "hauteur compensée par l'ottava : {note}"
    );
}

#[test]
fn transposition_chromatique_sans_controle_invalide() {
    let abc = ecrire("<transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose>");
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(
        v1.contains("transpose=-2"),
        "transposition chromatique : {v1}"
    );
    // Une transposition nulle ne doit pas produire de champ transpose=.
    let abc = ecrire("<transpose><diatonic>0</diatonic><chromatic>0</chromatic></transpose>");
    let v1 = abc.lines().find(|l| l.starts_with("V:1 ")).expect("V:1");
    assert!(!v1.contains("transpose="), "transposition nulle : {v1}");
}
