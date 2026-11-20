// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/compatmidirender.cpp`
// (`CompatMidiRender::renderNoteArticulation`, 668-810) et de la table
// `excursions` de `compatmidirenderinternal.h:80-100` de MuseScore Studio,
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

//! Lecture MIDI d'un trille (`<Articulation>`/`<Ornament>` `ornamentTrill`).
//!
//! MuseScore 4.7.4 rend l'ornement par une excursion de 32e (`_32nd`, 60
//! ticks à 480/noire) : préfixe `{0,1}`, corps `{0,1}` répété autant que la
//! durée le permet, la dernière note tenant le reste. Le pas 0/1 est un degré
//! diatonique : do → ré (2 demi-tons), si → do (1 demi-ton).
//!
//! Attendus mesurés : partition réduite `ornamentTrill` sur un do4 en noire
//! à 2 battements/s, exportée par MuseScore 4.7.4 (`mscore -o ref.mid`) :
//! huit notes alternatives 60/74 puis les trois notes suivantes ordinaires.
//! Aucune fixture binaire n'est versionnée.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// Attaques `(tick, hauteur, durée)` du SMF.
fn notes(bytes: &[u8]) -> Vec<(i64, u8, i64)> {
    let smf = Smf::parse(bytes).expect("SMF valide");
    let mut result = Vec::new();
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
                } else if let Some(position) = open.iter().position(|(pitch, _)| *pitch == key) {
                    let (pitch, start) = open.remove(position);
                    result.push((start, pitch, tick - start));
                }
            }
        }
        assert!(open.is_empty(), "note non éteinte");
    }
    result.sort_unstable();
    result
}

fn mscx(trill: &str) -> String {
    format!(
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
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
          <Chord>
            <durationType>quarter</durationType>
            {trill}
            <Note><pitch>72</pitch><tpc>14</tpc></Note>
          </Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
}

#[test]
fn trille_alterne_le_degre_diatonique_superieur() {
    for (label, trill) in [
        (
            "Articulation",
            "<Articulation><subtype>ornamentTrill</subtype></Articulation>",
        ),
        (
            "Ornement",
            "<Ornament><subtype>ornamentTrill</subtype></Ornament>",
        ),
    ] {
        let container = MsczContainer::from_bytes(mscx(trill).as_bytes())
            .unwrap_or_else(|err| panic!("{label}: conteneur : {err}"));
        let score = scoretk_musescore_import::read::read_score(&container)
            .unwrap_or_else(|err| panic!("{label}: lecture : {err}"));
        let midi = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
        let got = notes(&midi);
        let expected: Vec<(i64, u8, i64)> = (0..8)
            .map(|index| (index * 60, if index % 2 == 0 { 72 } else { 74 }, 59))
            .chain([(480, 60, 479), (960, 62, 479), (1440, 64, 479)])
            .collect();
        assert_eq!(got, expected, "{label}: trille joué");
    }
}
