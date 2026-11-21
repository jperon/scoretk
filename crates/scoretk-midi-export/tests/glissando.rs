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

//! Glissando chromatique : `CompatMidiRender::renderGlissando`.
//!
//! `renderGlissando` (`compatmidirender.cpp:646-658`) appelle
//! `renderNoteArticulation` avec `Glissando::pitchSteps`
//! (`engraving/dom/glissando.cpp:162-248`) ; le style par défaut est
//! `CHROMATIC` et la course chromatique occupe le dernier tiers de l'accord
//! (`glissandoPart = 0.33`, `compatmidirender.cpp:795-870`). La dernière note
//! est tenue jusqu'à la fin de l'accord ; la note d'arrivée est jouée par son
//! propre accord.
//!
//! Valeurs attendues : MuseScore 4.7.4 sur le `.mscx` 2.06 de ce test (une
//! noire 64 avec glissando vers 81 à la mesure suivante).

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="2.06">
  <Score>
    <Division>480</Division>
    <Part><Staff id="1"/><trackName>Accordion</trackName>
      <Instrument id="free-reed.accordion"><longName>Accordion</longName><shortName>Acc.</shortName><trackName>Accordion</trackName>
        <instrumentId>free-reed.accordion</instrumentId><Channel><program value="21"/></Channel></Instrument></Part>
    <Staff id="1">
      <Measure number="1">
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
        <Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
        <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>18</tpc>
          <Glissando id="9"><text>gliss.</text><subtype>1</subtype><diagonal>1</diagonal><lineWidth>0.15</lineWidth><anchor>3</anchor></Glissando>
          </Note></Chord>
      </Measure>
      <Measure number="2">
        <Chord><durationType>quarter</durationType><Note><pitch>81</pitch><tpc>17</tpc><endSpanner id="9"></endSpanner></Note></Chord>
        <Chord><durationType>quarter</durationType><Note><pitch>76</pitch><tpc>18</tpc></Note></Chord>
        <Chord><durationType>half</durationType><Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
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
fn un_glissando_chromatique_remplit_le_dernier_tiers() {
    let mut attendu = vec![(60, 0), (62, 480), (64, 960)];
    // 65..80 : course chromatique, 318 puis 328, 338… 466 ticks après 960.
    for (index, tick) in [
        1278u32, 1288, 1298, 1308, 1318, 1327, 1337, 1347, 1357, 1367, 1376, 1386, 1396, 1406,
        1416, 1426,
    ]
    .into_iter()
    .enumerate()
    {
        attendu.push((65 + index as u8, tick));
    }
    attendu.push((81, 1920));
    attendu.push((76, 2400));
    attendu.push((72, 2880));
    assert_eq!(attaques(mscx()), attendu);
}
