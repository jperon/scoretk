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

//! Vélocité utilisateur d'une note (`<velocity>` / `<veloType>`).
//!
//! En 4.x, le lecteur suit `rw/read410/tread.cpp:3174` (`velocity` →
//! `setUserVelocity`) et `:3189-3190` (`veloType` → `Pid::VELO_TYPE`). En 3.x,
//! ces balises ne sont pas lues, parité avec le C++ (`read302`).

use mscz2xml::model::{Note, VeloType};
use mscz2xml::read::read_score;
use mscz2xml::MsczContainer;

/// `.mscx` minimal d'une partie de piano, avec le contenu de `<voice>` fourni.
fn mscx(version: &str, voice: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="{version}">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
      </Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName>
        <shortName>Pno.</shortName>
        <trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
{voice}
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
}

fn chord(notes: &str) -> String {
    format!("          <Chord><durationType>quarter</durationType>{notes}</Chord>\n")
}

fn note(pitch: u8, tpc: i8, extra: &str) -> String {
    format!("<Note><pitch>{pitch}</pitch><tpc>{tpc}</tpc>{extra}</Note>")
}

fn lire(version: &str, voice: &str) -> Vec<Note> {
    let source = mscx(version, voice);
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = read_score(&container).expect("lecture du .mscx");
    score.notes
}

#[test]
fn lit_les_velocites_utilisateur_en_4x() {
    let voice = [
        chord(&note(
            60,
            14,
            "<velocity>50</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            61,
            14,
            "<velocity>-50</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            62,
            16,
            "<velocity>42</velocity><veloType>user</veloType>",
        )),
        // Sans `<veloType>`, la propriété vaut `USER_VAL` par défaut
        // (`dom/note.cpp:3316-3317`) : `<velocity>` est alors absolue.
        chord(&note(63, 16, "<velocity>30</velocity>")),
        // Une valeur inconnue retombe sur `OFFSET_VAL`
        // (`rw/read410/tread.cpp:401-402`).
        chord(&note(
            64,
            18,
            "<velocity>10</velocity><veloType>inconnu</veloType>",
        )),
    ]
    .concat();
    let notes = lire("4.10", &voice);
    assert_eq!(notes.len(), 5);

    assert_eq!(notes[0].user_velocity, Some(50));
    assert_eq!(notes[0].velo_type, Some(VeloType::OffsetVal));
    assert_eq!(notes[1].user_velocity, Some(-50));
    assert_eq!(notes[1].velo_type, Some(VeloType::OffsetVal));
    assert_eq!(notes[2].user_velocity, Some(42));
    assert_eq!(notes[2].velo_type, Some(VeloType::UserVal));
    assert_eq!(notes[3].user_velocity, Some(30));
    assert_eq!(notes[3].velo_type, None);
    assert_eq!(notes[4].user_velocity, Some(10));
    assert_eq!(notes[4].velo_type, Some(VeloType::OffsetVal));

    // `Note::velocity` reste réservé à l'import MIDI.
    assert!(notes.iter().all(|note| note.velocity.is_none()));
}

#[test]
fn ignore_les_velocites_utilisateur_en_3x() {
    let voice = [
        chord(&note(
            60,
            14,
            "<velocity>50</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            61,
            14,
            "<velocity>-50</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            62,
            16,
            "<velocity>42</velocity><veloType>user</veloType>",
        )),
    ]
    .concat();
    let notes = lire("3.02", &voice);
    assert_eq!(notes.len(), 3);
    assert!(notes.iter().all(|note| note.user_velocity.is_none()));
    assert!(notes.iter().all(|note| note.velo_type.is_none()));
    assert!(notes.iter().all(|note| note.velocity.is_none()));
}
