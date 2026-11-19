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

//! Écriture ABC des silences de mesure courte.
//!
//! Port de `xml2abc.py::doMaat` (ligne 674) et des règles de mesure/barre de
//! `abc_grammar` (lignes 428-436) : `Z` est un silence de **mesure entière**
//! pour `abc2midi`, dont la durée vient de l'indication `M:` courante. Une
//! mesure écrite plus courte que `M:` doit donc être remplie par un `z`
//! explicite, sinon toutes les attaques suivantes sont décalées.

#![cfg(all(feature = "musicxml-import", feature = "abc-export"))]

use scoretk::{write, Format, Options};

/// Deux portées : musique ordinaire (accord, silences, barre finale),
/// silence court de 3/4, silence court de 2/4 en fin de partition, et un
/// silence de mesure pleine qui sert de témoin.
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
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>\
<note><rest/><duration>2</duration><type>quarter</type></note>\
</measure>\
<measure number=\"2\">\
<note><rest measure=\"yes\"/><duration>6</duration></note>\
</measure>\
<measure number=\"3\">\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
<note><pitch><step>A</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
</measure>\
<measure number=\"4\">\
<note><rest measure=\"yes\"/><duration>8</duration></note>\
<barline location=\"right\"><bar-style>light-heavy</bar-style></barline>\
</measure>\
</part>\
<part id=\"P2\">\
<measure number=\"1\">\
<attributes><divisions>2</divisions><key><fifths>0</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time>\
<clef><sign>G</sign><line>2</line></clef></attributes>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>half</type></note>\
</measure>\
<measure number=\"2\">\
<note><rest measure=\"yes\"/><duration>4</duration></note>\
</measure>\
<measure number=\"3\">\
<note><rest measure=\"yes\"/><duration>8</duration></note>\
</measure>\
<measure number=\"4\">\
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
fn silence_court_est_un_z_explicite_pas_un_z_majuscule() {
    let abc = ecrire();
    let v1 = voix(&abc, 1);
    assert_eq!(v1.len(), 4, "quatre mesures écrites : {v1:?}");
    // Le silence de 3/4 est `z6` avec L:1/8 ; `Z` vaudrait 4/4 et décalerait
    // la troisième mesure d'un quart.
    assert_eq!(v1[1], "z6 |", "silence court : {v1:?}");
    // Musique ordinaire inchangée : accord, durées, silences.
    assert_eq!(v1[0], "[CE]2G2z2x2 |", "mesure ordinaire : {v1:?}");
    assert_eq!(v1[2], "G4A4 |", "durées : {v1:?}");
    // Cas « non concerné » : une vraie mesure pleine reste `Z`.
    assert_eq!(v1[3], "Z |]", "silence de mesure pleine : {v1:?}");
}

#[test]
fn silence_court_en_fin_de_fichier_et_dans_une_autre_voix() {
    let abc = ecrire();
    let v2 = voix(&abc, 2);
    assert_eq!(v2.len(), 4, "quatre mesures dans la voix 2 : {v2:?}");
    // Mesure courte dans une autre voix : `z` explicite, jamais `Z`.
    assert_eq!(v2[1], "z6 |", "silence court de la voix 2 : {v2:?}");
    // Les mesures pleines de la même voix restent intactes, y compris en fin
    // de fichier (cas limite).
    assert_eq!(v2[0], "D4E4 |", "voix 2 mesure ordinaire : {v2:?}");
    assert_eq!(v2[2], "Z |", "témoin mesure pleine : {v2:?}");
    assert_eq!(v2[3], "Z |]", "témoin mesure pleine finale : {v2:?}");
}
