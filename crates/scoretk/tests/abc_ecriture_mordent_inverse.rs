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

//! Ornement « mordent inversé » à l'écriture (§ 4.14 de la norme ABC 2.1).
//!
//! La liste des symboles définis par § 4.14 contient `!uppermordent!` (et
//! l'abréviation `P`) mais pas `!invertedmordent!` ; « if an unimplemented or
//! unknown symbol is found, it should be ignored », ce que fait `abc2midi`
//! (« instruction !invertedmordent! ignored »). `xml2abc.py` mappe
//! `ornaments/inverted-mordent` sur `P` (`note_ornamentation_map`, ligne 35),
//! c'est-à-dire `!uppermordent!` ; le mordent ordinaire reste `!mordent!`
//! (= `!lowermordent!`, ligne 34).

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

#[test]
fn mordent_inverse_est_un_symbole_defini_par_la_norme() {
    let source = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<score-partwise version=\"4.0\"><work><work-title>Essai</work-title></work>\
<part-list><score-part id=\"P1\"><part-name>Violon</part-name></score-part></part-list>\
<part id=\"P1\"><measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><ornaments><inverted-mordent /></ornaments></notations></note>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>\
<notations><ornaments><mordent /></ornaments></notations></note>\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
</measure></part></score-partwise>";
    let partition = scoretk::read(source.as_bytes(), Format::MusicXml).expect("lecture MusicXML");
    let octets = write(&partition, Format::Abc, &Options::default()).expect("écriture ABC");
    let abc = String::from_utf8(octets).expect("ABC UTF-8");
    // Hauteur, durée et nombre de mesures conservés ; seul le nom du symbole
    // change, et la voix témoin (mordent ordinaire) reste identique.
    assert!(
        abc.contains("!uppermordent!C2!mordent!D2E4 |]"),
        "mordent inversé : {abc}"
    );
    assert!(
        !abc.contains("!invertedmordent!"),
        "symbole inconnu : {abc}"
    );
}
