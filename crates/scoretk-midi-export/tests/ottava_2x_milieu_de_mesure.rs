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

//! Ottava 2.x refermée en milieu de mesure.
//!
//! Dans un fichier 2.06, `<Ottava id>` et `<endSpanner id>` peuvent tomber
//! entre deux accords (`docs/agents/mscx-format.md` §19.8). `readMeasure206`
//! (`read206.cpp:2834-2852`) ferme alors le spanner au tick courant, pas au
//! début de la mesure. La référence est le MIDI exporté par MuseScore 4.7.4 de
//! cette partition réduite, sans fixture binaire.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Flute</trackName>
  <Instrument id="wind.flutes.flute"><longName>Flute</longName>
    <trackName>Flute</trackName><instrumentId>wind.flutes.flute</instrumentId>
    <Channel><program value="73"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure number="1">
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    <Ottava id="1"><subtype>8va</subtype></Ottava>
    <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>65</pitch><tpc>13</tpc></Note></Chord>
    </Measure>
  <Measure number="2">
    <Chord><durationType>quarter</durationType><Note><pitch>67</pitch><tpc>20</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>69</pitch><tpc>22</tpc></Note></Chord>
    <endSpanner id="1"/>
    <Chord><durationType>quarter</durationType><Note><pitch>71</pitch><tpc>24</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>72</pitch><tpc>19</tpc></Note></Chord>
    </Measure>
  </Staff>
</Score></museScore>"#;

#[test]
fn ottava_2x_se_ferme_sur_la_note_suivante_le_endspanner() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 2.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&bytes).expect("SMF valide");
    let mut notes = Vec::new();
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
                    notes.push((tick / 240, key.as_int()));
                }
            }
        }
    }
    notes.sort_unstable();
    // Le 8va couvre E4, F4, G4, A4 (d'où 76, 77, 79, 81) ; B4 et C5 restent
    // à leur hauteur écrite (71, 72). Un ancrage de `<endSpanner>` au début de
    // la deuxième mesure aurait laissé 67 et 69 à la place de 79 et 81.
    assert_eq!(
        notes,
        vec![
            (0, 60),
            (2, 62),
            (4, 76),
            (6, 77),
            (8, 79),
            (10, 81),
            (12, 71),
            (14, 72),
        ]
    );
}
