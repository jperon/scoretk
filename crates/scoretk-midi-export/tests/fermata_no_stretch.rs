// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test consigne le comportement de MuseScore 4.7.4 vérifié sur une
// partition réduite : `Fermata::timeStretch` (Pid::TIME_STRETCH,
// `engraving/dom/fermata.cpp:138-139, 250`) n'est pas utilisé par le rendu
// MIDI de compatibilité (`engraving/compat/midi/compatmidirender*.cpp`) :
// une noire sous point d'orgue garde sa durée d'une noire. Aucun port n'est
// donc nécessaire dans `scoretk-midi-export`.
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

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

const SOURCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1"><Measure><voice>
      <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
      <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
      <Chord>
        <durationType>quarter</durationType>
        <Articulation><subtype>fermataAbove</subtype></Articulation>
        <Note><pitch>72</pitch><tpc>14</tpc></Note>
      </Chord>
      <Chord><durationType>quarter</durationType><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
      <Chord><durationType>quarter</durationType><Note><pitch>76</pitch><tpc>18</tpc></Note></Chord>
      <Chord><durationType>quarter</durationType><Note><pitch>77</pitch><tpc>13</tpc></Note></Chord>
    </voice></Measure></Staff>
  </Score>
</museScore>
"#;

#[test]
fn point_dorgue_ne_change_pas_le_midi() {
    let container = MsczContainer::from_bytes(SOURCE.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let midi = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&midi).expect("SMF valide");
    let mut notes = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        let mut open: Vec<(u8, i64)> = Vec::new();
        for event in track {
            tick += i64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                let key = key.as_int();
                if vel.as_int() > 0 {
                    open.push((key, tick));
                } else if let Some(index) = open.iter().position(|(pitch, _)| *pitch == key) {
                    let (pitch, start) = open.remove(index);
                    notes.push((start, pitch, tick - start));
                }
            }
        }
        assert!(open.is_empty(), "note non éteinte");
    }
    notes.sort_unstable();
    let expected: Vec<(i64, u8, i64)> = [
        (0, 72, 479),
        (480, 74, 479),
        (960, 76, 479),
        (1440, 77, 479),
    ]
    .into_iter()
    .collect();
    assert_eq!(notes, expected, "durée MIDI sous point d'orgue");
}
