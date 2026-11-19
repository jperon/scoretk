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

//! Jeton ABC des articulations et nuances, pour rester silencieux sous abc2midi.
//!
//! Port de `xml2abc.py::note_ornamentation_map` (lignes 32-57) et
//! `dynamics_map` (lignes 59-71) : staccato → `.`, staccatissimo → `!wedge!`,
//! accent → `!>!`, strong-accent → `!^!`, et seules les nuances de la table
//! (`sfz` compris, `sf`/`fz` exclus) sont écrites comme `!nom!`. Les autres
//! noms (`!staccato!`, `!marcato!`, `!sf!`…) déclenchent autant
//! d'avertissements `abc2midi` par note.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

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
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><articulations><staccato /></articulations></notations></note>\
<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><articulations><accent /></articulations></notations></note>\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><articulations><strong-accent /></articulations></notations></note>\
<note><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><articulations><staccatissimo /></articulations></notations></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>A</step><octave>4</octave></pitch><duration>4</duration><type>half</type>\
<notations><articulations><tenuto /></articulations></notations></note>\
<note><rest/><duration>2</duration><type>quarter</type></note>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
</measure>\
</part>\
<part id=\"P2\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<direction><direction-type><dynamics><sfz /></dynamics></direction-type></direction>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
<direction><direction-type><dynamics><sf /></dynamics></direction-type></direction>\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
</measure>\
<measure number=\"2\">\
<note><rest measure=\"yes\"/><duration>8</duration></note>\
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

#[test]
fn articulations_sont_les_jetons_de_xml2abc() {
    let abc = ecrire();
    let m1 = abc
        .lines()
        .find(|l| l.contains("[CE]2"))
        .unwrap_or("");
    assert!(abc.contains(".[CE]2"), "staccato doit être '.' : {abc}");
    assert!(abc.contains("!>!.[CE]2"), "accent puis staccato : {abc}");
    assert!(abc.contains("!^!G2"), "strong-accent doit être '!^!' : {abc}");
    assert!(abc.contains("!wedge!B2"), "staccatissimo doit être '!wedge!' : {abc}");
    assert!(!abc.contains("!staccato!"), "jeton ignoré par abc2midi : {abc}");
    assert!(!abc.contains("!accent!"), "jeton ignoré par abc2midi : {abc}");
    assert!(!abc.contains("!marcato!"), "jeton ignoré par abc2midi : {abc}");
    // Cas « non concerné » : tenuto garde son nom long, reconnu par abc2midi.
    assert!(abc.contains("!tenuto!A4"), "tenuto inchangé : {abc}");
    assert!(m1.contains('|'), "mesure ordinaire conservée : {m1}");
}

#[test]
fn nuances_non_tablees_ne_sont_pas_des_decorations() {
    let abc = ecrire();
    assert!(abc.contains("!sfz!"), "sfz est dans dynamics_map : {abc}");
    assert!(!abc.contains("!sf!"), "sf n'est pas dans dynamics_map : {abc}");
    // Les deux voix et leurs deux mesures restent présentes (régression).
    assert_eq!(abc.lines().filter(|l| l.starts_with("V:")).count(), 2);
    assert!(abc.contains("Z |]"), "silence de mesure pleine témoin : {abc}");
}
