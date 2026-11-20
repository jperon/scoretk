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

//! Déroulement 2.x : marqueurs, sauts et voltas à plusieurs fins.
//!
//! La syntaxe 2.x (`<Marker>`, `<Jump>`, `<Volta id>`/`<endSpanner id>`, sans
//! bloc `<voice>`) est décrite dans `docs/agents/mscx-format.md` §19. Le
//! déroulement est porté de `RepeatList::unwind`
//! (`engraving/dom/repeatlist.cpp:787-1069`). La référence est le MIDI exporté
//! par MuseScore **4.7.4** de cette même partition réduite ; la norme SMF ne
//! conservant ni reprises ni sauts (§"Instruments, parts, repeats" de
//! `docs/agents/format-ref/midi.md`), c'est le comportement de l'outil qui
//! fait foi. Aucun binaire n'est versionné : seules les attaques sont figées.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// Partition 2.06 réduite construite à la main : `segno` en m1, reprise en m2,
/// volta `1, 3` en m3 fermée en m4, volta `2, 4` en m5, D.C. en m5 et `fine`
/// en m6. Aucune donnée tierce.
const SCORE: &str = r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Flute</trackName>
  <Instrument id="wind.flutes.flute"><longName>Flute</longName>
    <trackName>Flute</trackName><instrumentId>wind.flutes.flute</instrumentId>
    <Channel><program value="73"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure number="1">
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Marker><label>segno</label><text>segno</text></Marker>
    <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>65</pitch><tpc>13</tpc></Note></Chord>
    </Measure>
  <Measure number="2">
    <startRepeat/>
    <Chord><durationType>quarter</durationType><Note><pitch>67</pitch><tpc>20</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>69</pitch><tpc>22</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>71</pitch><tpc>24</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>72</pitch><tpc>19</tpc></Note></Chord>
    </Measure>
  <Measure number="3">
    <Volta id="1"><endHook>1</endHook><endings>1, 3</endings></Volta>
    <Chord><durationType>quarter</durationType><Note><pitch>74</pitch><tpc>21</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>76</pitch><tpc>23</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>77</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>79</pitch><tpc>20</tpc></Note></Chord>
    </Measure>
  <Measure number="4">
    <endSpanner id="1"/>
    <endRepeat>4</endRepeat>
    <Chord><durationType>quarter</durationType><Note><pitch>72</pitch><tpc>19</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>71</pitch><tpc>24</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>69</pitch><tpc>22</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>67</pitch><tpc>20</tpc></Note></Chord>
    </Measure>
  <Measure number="5">
    <Volta id="2"><endings>2, 4</endings></Volta>
    <Jump><jumpTo>segno</jumpTo><playUntil>fine</playUntil></Jump>
    <Chord><durationType>quarter</durationType><Note><pitch>65</pitch><tpc>13</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    </Measure>
  <Measure number="6">
    <Marker><label>fine</label><text>fine</text></Marker>
    <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    </Measure>
  </Staff>
</Score></museScore>"#;

/// Attaques (en demi-noires) et hauteurs du MIDI produit.
fn attacks(bytes: &[u8]) -> Vec<(i64, u8)> {
    let smf = Smf::parse(bytes).expect("SMF valide");
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
fn voltas_multi_fins_et_dc_sont_deplies_comme_musescore() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 2.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let got = attacks(&bytes);
    let expected = [
        (0, 60),
        (2, 62),
        (4, 64),
        (6, 65),
        (8, 67),
        (10, 69),
        (12, 71),
        (14, 72),
        (16, 74),
        (18, 76),
        (20, 77),
        (22, 79),
        (24, 72),
        (26, 71),
        (28, 69),
        (30, 67),
        (32, 67),
        (34, 69),
        (36, 71),
        (38, 72),
        (40, 72),
        (42, 71),
        (44, 69),
        (46, 67),
        (48, 67),
        (50, 69),
        (52, 71),
        (54, 72),
        (56, 74),
        (58, 76),
        (60, 77),
        (62, 79),
        (64, 72),
        (66, 71),
        (68, 69),
        (70, 67),
        (72, 67),
        (74, 69),
        (76, 71),
        (78, 72),
        (80, 72),
        (82, 71),
        (84, 69),
        (86, 67),
        (88, 65),
        (90, 64),
        (92, 62),
        (94, 60),
        (96, 60),
        (98, 62),
        (100, 64),
        (102, 65),
        (104, 67),
        (106, 69),
        (108, 71),
        (110, 72),
        (112, 72),
        (114, 71),
        (116, 69),
        (118, 67),
        (120, 65),
        (122, 64),
        (124, 62),
        (126, 60),
        (128, 60),
    ];
    assert_eq!(got, expected, "attaques 2.x (référence MuseScore 4.7.4)");
}
