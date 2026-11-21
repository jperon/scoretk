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

//! Ornement désactivé (`<play>0</play>`) : il reste écrit mais ne joue pas.
//!
//! `Articulation::playArticulation()` (`engraving/dom/articulation.h`) est lu
//! par `renderArticulation` (`compatmidirender.cpp:483-490`) : une
//! articulation dont `play` est faux est ignorée. `Dominus_fredipi` porte trois
//! `<Articulation><subtype>ornamentTrill</subtype><play>0</play>` que le port
//! jouait quand même.
//!
//! Valeurs attendues : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test, une
//! blanche trillée muette puis une blanche normale : deux notes seulement.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId><Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>half</durationType>
            <Articulation><subtype>ornamentTrill</subtype><play>0</play></Articulation>
            <Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>half</durationType><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Couples (hauteur, attaque) des notes du SMF produit, en ticks.
fn attaques(mscx: &str) -> Vec<(u8, u32)> {
    let conteneur = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF valide");
    let mut resultat = Vec::new();
    for piste in &smf.tracks {
        let mut tick = 0u32;
        for evenement in piste {
            tick += evenement.delta.as_int();
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = evenement.kind
            {
                if vel.as_int() > 0 {
                    resultat.push((key.as_int(), tick));
                }
            }
        }
    }
    resultat.sort_by_key(|(_, attaque)| *attaque);
    resultat
}

#[test]
fn un_ornement_desactive_ne_joue_pas() {
    assert_eq!(attaques(mscx()), vec![(72, 0), (74, 960)]);
}
