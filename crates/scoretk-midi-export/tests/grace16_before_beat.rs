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

//! Note d'agrément de valeur (`GRACE4`/`GRACE16`/`GRACE32`) : avant le temps.
//!
//! MuseScore place ces ornements **avant** la note principale et ne retire pas
//! leur durée à celle-ci (`createGraceNotesPlayEvents`,
//! `engraving/compat/midi/compatmidirender.cpp:559-590` ; la position vient du
//! tick de l'accord d'agrément, posé avant le temps par la gravure). Le
//! lecteur les traitait comme des appoggiatures, sur le temps et en
//! raccourcissant la note principale, ce qui décalait la main de
//! `Sancta_Maria_Mater_Dei` (Mozart, `.mscz` 3.02).
//!
//! Valeurs attendues : une noire 72 (0–479), une grâce de double croche 74 sur
//! les 120 ticks qui précèdent le temps (360–479), puis la noire 76 pleine
//! (480–959).

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
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
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>quarter</durationType><Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>16th</durationType><grace16/><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>76</pitch><tpc>18</tpc></Note></Chord>
          <Chord><durationType>half</durationType><Note><pitch>77</pitch><tpc>13</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

fn notes(mscx: &str) -> Vec<(u8, u32, u32)> {
    let conteneur = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF valide");
    let mut resultat = Vec::new();
    for piste in &smf.tracks {
        let mut tick = 0u32;
        let mut attaques: Vec<(u8, u32)> = Vec::new();
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
                        resultat.push((hauteur, attaque, tick));
                    }
                }
                _ => {}
            }
        }
    }
    resultat.sort_by_key(|(_, attaque, _)| *attaque);
    resultat
}

#[test]
fn grace16_avant_le_temps_et_note_pleine() {
    assert_eq!(
        notes(mscx()),
        vec![
            (72, 0, 479),
            (74, 360, 479),
            (76, 480, 959),
            (77, 960, 1919),
        ]
    );
}
