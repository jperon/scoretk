// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `CompatMidiRendererInternal::collectNote` et de
// `chromaticPitchSteps` (`engraving/dom/utils.cpp:807`) de MuseScore Studio,
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

//! Trille porté par une note tenue, avec acciaccatura liée.
//!
//! Dans `Minuit chrétien`, l'ornement est écrit sur la note principale, liée en
//! arrière à une acciaccatura et en avant à la note suivante. MuseScore attache
//! les évènements de jeu à la note de départ de la liaison
//! (`createPlayEvents`) et `graceNotesMerged` empêche de réémettre
//! l'acciaccatura : le trille part de la note principale et couvre toute la
//! chaîne. À l'inverse, `chromaticPitchSteps` (`dom/utils.cpp:807`) cherche
//! l'accident le plus récent de la même ligne dans la mesure : le mi bémol de
//! la mesure rend l'auxiliaire du ré bémol (75), pas mi naturel (76).
//!
//! Attendus : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Violin</trackName>
      <Instrument id="strings.violin">
        <longName>Violin</longName><shortName>Vln.</shortName><trackName>Violin</trackName>
        <instrumentId>strings.violin</instrumentId>
        <Channel><program value="40"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure><voice>
        <TimeSig><sigN>12</sigN><sigD>8</sigD></TimeSig>
        <Tempo><tempo>1.5</tempo><followText>1</followText></Tempo>
        <Chord><dots>1</dots><durationType>half</durationType><Note><pitch>75</pitch><tpc>11</tpc></Note></Chord>
        <Chord><durationType>32nd</durationType><Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
        <Chord><durationType>32nd</durationType><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
        <Chord><durationType>32nd</durationType><Note><pitch>75</pitch><tpc>11</tpc></Note></Chord>
        <Chord><durationType>32nd</durationType><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>
        <Chord><durationType>eighth</durationType><acciaccatura />
          <Note>
            <Spanner type="Tie"><Tie></Tie><next><location></location></next></Spanner>
            <pitch>74</pitch><tpc>16</tpc>
          </Note>
        </Chord>
        <Chord><durationType>quarter</durationType>
          <Ornament><subtype>ornamentTrill</subtype></Ornament>
          <Note>
            <Spanner type="Tie"><Tie></Tie><next><location><fractions>1/4</fractions></location></next></Spanner>
            <Spanner type="Tie"><prev><location><grace>0</grace></location></prev></Spanner>
            <pitch>74</pitch><tpc>16</tpc>
          </Note>
        </Chord>
        <Chord><durationType>quarter</durationType>
          <Note>
            <Spanner type="Tie"><prev><location><fractions>-1/4</fractions></location></prev></Spanner>
            <pitch>74</pitch><tpc>16</tpc>
          </Note>
        </Chord>
        <Chord><durationType>16th</durationType><Note><pitch>75</pitch><tpc>11</tpc></Note></Chord>
        <Chord><durationType>16th</durationType><Note><pitch>77</pitch><tpc>13</tpc></Note></Chord>
      </voice></Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Attaques `(tick, hauteur)` du SMF produit.
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
fn trille_sur_note_tenue_avec_acciaccatura_liee() {
    let mut expected = vec![(0, 75), (1440, 72), (1500, 74), (1560, 75), (1620, 74)];
    for (index, tick) in [
        1680i64, 1740, 1800, 1860, 1920, 1980, 2040, 2100, 2160, 2220, 2280, 2340, 2400, 2460,
    ]
    .into_iter()
    .enumerate()
    {
        expected.push((tick, if index % 2 == 0 { 74 } else { 75 }));
    }
    expected.push((2640, 75));
    expected.push((2760, 77));
    assert_eq!(attacks(mscx()), expected, "trille tenu et auxiliaire bémol");
}
