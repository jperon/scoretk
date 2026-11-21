// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `CompatMidiRender::renderArpeggio`
// (`engraving/compat/midi/compatmidirender.cpp`) de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Arpège : `renderArpeggio` (`compatmidirender.cpp:283-313`).
//!
//! Le pas vaut 64 abaissé par `2/3` tant que `pas * notes > playTicks`, puis
//! chaque note est retardée de `pas * rang * 1000 / playTicks` millièmes, sans
//! `gateTime` : toutes finissent à la fin de l'accord. `playTicks` est la
//! durée liée de la note supérieure (`Note::playTicks`).
//!
//! Attendus : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test (accord de quatre
//! notes, blanche 960 ticks, tempo 120).

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
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure><voice>
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
        <Chord><durationType>half</durationType>
          <Arpeggio><subtype>0</subtype></Arpeggio>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
          <Note><pitch>64</pitch><tpc>18</tpc></Note>
          <Note><pitch>67</pitch><tpc>15</tpc></Note>
          <Note><pitch>72</pitch><tpc>14</tpc></Note>
        </Chord>
        <Chord><durationType>half</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </voice></Measure>
    </Staff>
  </Score>
</museScore>
"#
}

fn attacks(mscx: &str) -> Vec<(i64, u8)> {
    let container = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&bytes).expect("SMF valide");
    let mut out = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        for event in track {
            tick += i64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                if vel.as_int() > 0 {
                    out.push((tick, key.as_int()));
                }
            }
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn arpege_repartit_les_notes_sur_la_duree_de_l_accord() {
    assert_eq!(
        attacks(mscx()),
        vec![(0, 60), (63, 64), (127, 67), (192, 72), (960, 60)]
    );
}
