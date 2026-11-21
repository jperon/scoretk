// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `RepeatList::findMarker`, `RepeatList::performJump` et
// `RepeatList::unwind` (`engraving/dom/repeatlist.cpp`) de MuseScore Studio,
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

//! Deux sauts de fin avec `<playRepeats>1</playRepeats>`.
//!
//! `Jump::playRepeats` (`dom/jump.h:68,85`) décide si un saut rejoue les
//! reprises de sa cible : `performJump` reçoit `withRepeats` et `unwind` pose
//! `forceFinalRepeat = !playRepeats` (`repeatlist.cpp:985-993`). Quand il vaut
//! vrai, la reprise de la cible est rejouée à chaque passage ; quand il vaut
//! faux, le saut force le dernier passage et la reprise n'est plus honorée.
//!
//! `Dieu_seul_suffit` porte deux sauts (`D.C.` vers `start`, `D.S.` vers
//! `segno`) avec `<playRepeats>1</playRepeats>` et une reprise de deux
//! mesures. La référence MuseScore 4.7.4 joue trois fois le bloc complet
//! `m0-5 + reprise m4-5` ; sans `playRepeats`, le lecteur n'en joue que deux
//! fois et demie.
//!
//! Attendus : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test (une ronde par
//! mesure, six mesures, reprise de m4-5, deux sauts avec `playRepeats`).

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
      <Measure>
        <Marker><label>segno</label><markerType>segno</markerType></Marker>
        <voice><TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice>
      </Measure>
      <Measure><voice><Chord><durationType>whole</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></voice></Measure>
      <Measure><voice><Chord><durationType>whole</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord></voice></Measure>
      <Measure><voice><Chord><durationType>whole</durationType><Note><pitch>65</pitch><tpc>13</tpc></Note></Chord></voice></Measure>
      <Measure>
        <startRepeat/>
        <voice><Chord><durationType>whole</durationType><Note><pitch>67</pitch><tpc>15</tpc></Note></Chord></voice>
      </Measure>
      <Measure>
        <endRepeat>2</endRepeat>
        <Jump><jumpTo>start</jumpTo><playUntil>end</playUntil><continueAt></continueAt><playRepeats>1</playRepeats></Jump>
        <Jump><jumpTo>segno</jumpTo><playUntil>end</playUntil><continueAt></continueAt><playRepeats>1</playRepeats></Jump>
        <voice><Chord><durationType>whole</durationType><Note><pitch>69</pitch><tpc>17</tpc></Note></Chord></voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Attaques `(tick en croches, hauteur)` du SMF produit.
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
                    out.push((tick / 240, key.as_int()));
                }
            }
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn deux_sauts_avec_play_repeats_rejouent_la_reprise() {
    let mut expected = Vec::new();
    for pass in 0..3 {
        let base = pass * 64;
        for (index, pitch) in [60u8, 62, 64, 65, 67, 69, 67, 69].into_iter().enumerate() {
            expected.push((base + 8 * index as i64, pitch));
        }
    }
    assert_eq!(
        attacks(mscx()),
        expected,
        "trois blocs de six mesures et reprise"
    );
}
