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

//! Unisson sur deux portées d'une même partie : `EventsHolder::fixupMIDI`.
//!
//! `fixupMIDI` (`engraving/compat/midi/event.cpp:263-306`) compte les notes
//! jouées par (canal, hauteur). Quand une seconde note de même hauteur démarre
//! pendant que la première sonne encore, elle reçoit `discard = portée de la
//! précédente + 1` ; `exportmidi.cpp:304-323` écrit alors une extinction à son
//! tick sur la piste précédente (« restrike »), et l'extinction propre de la
//! première note est ignorée ou déplacée sur la piste de la dernière attaque.
//! Résultat : les deux notes ne se chevauchent jamais sur le canal.
//!
//! Valeurs attendues : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test (orgue,
//! deux portées, unisson de 55). Piste 1 : 55 de 0 à 0 (extinction immédiate),
//! 63 de 240 à 479, 58 de 480 à 959, 55 de 960 à 1919. Piste 2 : 55 de 0 à 960.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <Staff id="2"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Organ</trackName>
      <Instrument id="keyboard.pipeorgan">
        <longName>Organ</longName><shortName>Org.</shortName><trackName>Organ</trackName>
        <instrumentId>keyboard.pipeorgan</instrumentId><Channel><program value="19"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>eighth</durationType><Note><pitch>55</pitch><tpc>15</tpc></Note></Chord>
          <Chord><durationType>eighth</durationType><Note><pitch>63</pitch><tpc>11</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>58</pitch><tpc>12</tpc></Note></Chord>
          <Chord><durationType>half</durationType><Note><pitch>55</pitch><tpc>15</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
    <Staff id="2">
      <Measure>
        <voice>
          <Chord><durationType>whole</durationType><Note><pitch>55</pitch><tpc>15</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Notes `(hauteur, attaque, extinction)` par piste du SMF produit.
fn notes_par_piste(mscx: &str) -> Vec<Vec<(u8, u32, u32)>> {
    let conteneur = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF valide");
    let mut pistes = Vec::new();
    for piste in &smf.tracks {
        let mut tick = 0u32;
        let mut attaques: Vec<(u8, u32)> = Vec::new();
        let mut notes = Vec::new();
        for evenement in piste {
            tick += evenement.delta.as_int();
            match evenement.kind {
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, vel },
                    ..
                } if vel.as_int() > 0 => attaques.push((key.as_int(), tick)),
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, .. },
                    ..
                }
                | TrackEventKind::Midi {
                    message: MidiMessage::NoteOff { key, .. },
                    ..
                } => {
                    let hauteur = key.as_int();
                    if let Some(position) = attaques.iter().position(|(k, _)| *k == hauteur) {
                        let (_, attaque) = attaques.remove(position);
                        notes.push((hauteur, attaque, tick));
                    }
                }
                _ => {}
            }
        }
        notes.sort_by_key(|(_, attaque, _)| *attaque);
        pistes.push(notes);
    }
    pistes
}

#[test]
fn un_unisson_sur_deux_portees_est_restrike() {
    let pistes = notes_par_piste(mscx());
    assert_eq!(pistes.len(), 2);
    assert_eq!(
        pistes[0],
        vec![(55, 0, 0), (63, 240, 479), (58, 480, 959), (55, 960, 1919)]
    );
    assert_eq!(pistes[1], vec![(55, 0, 960)]);
}
