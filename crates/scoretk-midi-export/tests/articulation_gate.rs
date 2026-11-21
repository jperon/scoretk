// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `CompatMidiRender::updateGateTime`
// (`engraving/compat/midi/compatmidirender.cpp`) et de
// `Read206::articulationNames2SymId206` (`engraving/rw/read206/read206.cpp`)
// de MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Durée de tenue des articulations nommées à l'ancienne (`staccato`).
//!
//! Le format 2.x/3.x écrit `<Articulation><subtype>staccato</subtype>` ;
//! `Read206::articulationNames2SymId206` (`read206.cpp:594-652`) traduit ces
//! noms vers le symbole SMuFL avant de créer l'articulation. Sans cette
//! traduction, l'articulation reste `Other("staccato")`, `gate_time` ne
//! reconnaît pas le nom et la note garde le `gateTime` par défaut de
//! l'instrument.
//!
//! Attendus : MuseScore 4.7.4 sur le `.mscx` 2.06 de ce test (croche pointée
//! 62 staccato, puis croche 64 ; instrument à `gateTime` 95 %, staccato 50 %).

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="2.06">
  <Score>
    <Division>480</Division>
    <Part>
      <Staff id="1"/>
      <trackName>Piano</trackName>
      <Instrument>
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
        <Articulation><velocity>100</velocity><gateTime>95</gateTime></Articulation>
        <Articulation name="staccatissimo"><velocity>100</velocity><gateTime>33</gateTime></Articulation>
        <Articulation name="staccato"><velocity>100</velocity><gateTime>50</gateTime></Articulation>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure number="1">
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Chord><dots>1</dots><durationType>eighth</durationType>
          <Articulation><subtype>staccato</subtype></Articulation>
          <Note><pitch>62</pitch><tpc>16</tpc></Note>
        </Chord>
        <Chord><durationType>eighth</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Notes `(hauteur, attaque, durée)` du SMF produit.
fn notes(mscx: &str) -> Vec<(u8, u32, u32)> {
    let conteneur = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF valide");
    let mut resultat = Vec::new();
    for piste in &smf.tracks {
        let mut tick = 0u32;
        let mut ouvertes: Vec<(u8, u32)> = Vec::new();
        for evenement in piste {
            tick += evenement.delta.as_int();
            match evenement.kind {
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, vel },
                    ..
                } => {
                    let hauteur = key.as_int();
                    if vel.as_int() > 0 {
                        ouvertes.push((hauteur, tick));
                    } else if let Some(position) = ouvertes.iter().position(|(k, _)| *k == hauteur)
                    {
                        let (k, attaque) = ouvertes.remove(position);
                        resultat.push((k, attaque, tick - attaque));
                    }
                }
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOff { key, .. },
                    ..
                } => {
                    let hauteur = key.as_int();
                    if let Some(position) = ouvertes.iter().position(|(k, _)| *k == hauteur) {
                        let (k, attaque) = ouvertes.remove(position);
                        resultat.push((k, attaque, tick - attaque));
                    }
                }
                _ => {}
            }
        }
    }
    resultat.sort_unstable();
    resultat
}

#[test]
fn staccato_2x_raccourcit_la_note() {
    assert_eq!(
        notes(mscx()),
        vec![
            (62, 0, 179),   // croche pointée 360 ticks × 50 % − 1
            (64, 360, 227), // croche 240 ticks × 95 % − 1 (instrument)
        ]
    );
}
