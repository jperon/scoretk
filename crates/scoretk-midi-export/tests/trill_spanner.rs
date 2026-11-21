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

//! Trille sous forme de spanner (`<Spanner type="Trill">`).
//!
//! Portage de `CompatMidiRender::findFirstTrill`
//! (`engraving/compat/midi/compatmidirender.cpp:1010-1030`) et de son appel
//! dans `createPlayEvents` (`:480-482`) : le premier trille qui recouvre
//! l'accord, sur la même piste et avec `playSpanner()`, remplace la note
//! tenue par l'excursion de `renderNoteArticulation` (`:913-927`). Le trille
//! ne s'applique pas à l'accord qui porte la fin du spanner
//! (`findOverlapping(tick + 1, endTick - 1)`).
//!
//! Valeurs attendues : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test. La
//! blanche 72 est décomposée en 16 attaques de 32e (0, 59, 120, 179… 899) et
//! la blanche 74 hors du trille reste une note à 960.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// `.mscx` 4.10 minimal : deux blanches, un trille de la première à la seconde.
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
          <Spanner type="Trill">
            <Trill><subtype>trill</subtype></Trill>
            <next><location><fractions>1/2</fractions></location></next>
          </Spanner>
          <Chord><durationType>half</durationType><Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
          <Spanner type="Trill">
            <prev><location><fractions>-1/2</fractions></location></prev>
          </Spanner>
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
fn un_trille_spanner_decompose_chaque_accord_couvert() {
    let mut attendu = Vec::new();
    // Excursion par défaut (32e) : la position de chaque évènement suit
    // `ontime / actualTicks` puis `actualTicks * ontime / 1000`.
    for (index, tick) in [
        0u32, 59, 120, 179, 240, 299, 360, 419, 480, 539, 600, 659, 720, 779, 840, 899,
    ]
    .into_iter()
    .enumerate()
    {
        attendu.push((if index % 2 == 0 { 72 } else { 74 }, tick));
    }
    attendu.push((74, 960));
    assert_eq!(attaques(mscx()), attendu);
}
