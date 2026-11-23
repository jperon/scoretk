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

//! Notes d'agrément de valeur (`<grace16/>`, `<grace32/>`) : elles se jouent
//! **sur le temps** de l'accord, qui est retardé de la largeur du groupe ; la
//! note principale finit au même endroit que sans grâces.
//!
//! Port de `createGraceNotesPlayEvents` (`engraving/compat/midi/
//! compatmidirender.cpp:530-606`) : `ontime = min(500, actualTicks(grâce) /
//! actualTicks(accord) * 1000)` est transmis à `renderChord` (`:193-252`,
//! `NoteEvent(0, ontime, 1000 - trailtime)`), puis converti en ticks par
//! `collectNote` (`compatmidirenderinternal.cpp:861-862`). Les grâces sont
//! émises par `createGraceNotesPlayEvents` (`:591-606`) avec des `ontime` de
//! `0, graceDuration, …` ; elles ne sont donc pas « volées » à la note
//! précédente, et `trailtime += ontime` raccourcit la note principale.
//!
//! Références : MuseScore 4.7.4 (`mscore <partition>.mscx -o <fichier>.mid`),
//! PPQ 480. Une grâce16 seule devant une noire en 480 : grâce 480–599, note
//! 600–959. Deux grâces32 devant une noire en 480 : grâces 480–508 et
//! 509–537, note 540–959.

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
fn grace16_sur_le_temps_et_note_raccourcie() {
    assert_eq!(
        notes(mscx()),
        vec![
            (72, 0, 479),
            (74, 480, 599),
            (76, 600, 959),
            (77, 960, 1919),
        ]
    );
}

/// Partition à deux grâces32, pour la largeur de groupe divisée par `nb`.
fn mscx_deux_graces() -> &'static str {
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
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>32nd</durationType><grace32/><Note><pitch>74</pitch><tpc>20</tpc></Note></Chord>
          <Chord><durationType>32nd</durationType><grace32/><Note><pitch>72</pitch><tpc>18</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>70</pitch><tpc>18</tpc></Note></Chord>
          <Chord><durationType>half</durationType><Note><pitch>67</pitch><tpc>16</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

#[test]
fn deux_graces32_se_jouent_sur_le_temps() {
    assert_eq!(
        notes(mscx_deux_graces()),
        vec![
            (60, 0, 479),
            (74, 480, 508),
            (72, 509, 537),
            (70, 540, 959),
            (67, 960, 1919),
        ]
    );
}
