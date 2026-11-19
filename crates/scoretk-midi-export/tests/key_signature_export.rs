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

//! Drapeau mineur `mi` du méta `FF 59` à l'écriture.
//!
//! MuseScore 4.7.4 écrit toujours 0 (`exportmidi.cpp`, `0 /* major */`) : le
//! portage écrit le mode du modèle pour préserver l'armure d'un SMF lu avec
//! `FF 59 02 sf 01`. Le cas majeur, lui, doit rester identique.

use midly::{MetaMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// `.mscx` 4.10 minimal, avec une armure de trois bémols et trois noires.
fn mscx(mode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
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
          <KeySig><concertKey>-3</concertKey><mode>{mode}</mode></KeySig>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>63</pitch><tpc>16</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>67</pitch><tpc>18</tpc></Note></Chord>
          <Rest><durationType>quarter</durationType></Rest>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
}

/// Armures `(tick, altérations, mineur)` du SMF produit.
fn key_events(source: &str) -> Vec<(u32, i8, bool)> {
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture .mscx");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&bytes).expect("SMF valide");
    let mut result = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0;
        for event in track {
            tick += event.delta.as_int();
            if let TrackEventKind::Meta(MetaMessage::KeySignature(sharps, minor)) = event.kind {
                result.push((tick, sharps, minor));
            }
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn armure_mineure_ecrite_avec_le_mode_du_modele() {
    assert_eq!(
        key_events(&mscx("minor")),
        vec![(0, -3, true)],
        "le drapeau mi doit venir du modèle",
    );
    // Cas non concerné : le mode majeur garde mi = 0, comme MuseScore.
    assert_eq!(key_events(&mscx("major")), vec![(0, -3, false)]);
}
