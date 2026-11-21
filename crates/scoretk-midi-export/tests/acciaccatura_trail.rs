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

//! Acciaccatura : l'accord qui la précède lui réserve du temps.
//!
//! Portage de `CompatMidiRender::adjustTrailtime`
//! (`engraving/compat/midi/compatmidirender.cpp:1035-1078`) : quand l'accord
//! suivant porte une acciaccatura, le `trailtime` de l'accord courant augmente
//! de `min(première acciaccatura, moitié de l'accord courant)` ticks
//! (`createGraceNotesPlayEvents`, puis `renderChord` qui en déduit
//! `1000 - trailtime`). La note précédente est donc raccourcie pour laisser
//! place à l'ornement, qui se joue avant le temps de l'accord suivant.
//!
//! Valeurs attendues : MuseScore 4.7.4, `.mscx` 4.10 de ce test, aucun
//! `gateTime` explicite (100 %). La noire 72 dure 240 ticks au lieu de 480 ;
//! l'acciaccatura 74 occupe 240–479 ; la noire 76 garde 480–959.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// `.mscx` 4.10 minimal : noire, acciaccatura (croche), puis deux noires.
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
          <Chord><durationType>eighth</durationType><acciaccatura/><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>76</pitch><tpc>18</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>77</pitch><tpc>13</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>79</pitch><tpc>15</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Couples (hauteur, attaque, extinction) des notes du SMF produit, en ticks.
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
fn l_accord_avant_une_acciaccatura_est_raccourci() {
    let notes = notes(mscx());
    assert_eq!(
        notes,
        vec![
            (72, 0, 239),
            (74, 240, 479),
            (76, 480, 959),
            (77, 960, 1439),
            (79, 1440, 1919),
        ]
    );
}
