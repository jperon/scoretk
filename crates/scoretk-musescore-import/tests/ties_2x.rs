// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Read206::readScoreFile` (`engraving/rw/read206/read206.cpp`) de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
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

//! Liaison 2.x après un accord d'agrément : les rangs de notes doivent ignorer
//! les grâces, qui ne sont pas des `ChordRest` du modèle.
//!
//! `fix_ties` retrouve les notes par `(mesure, piste, rang)` ; le rang est
//! celui des notes des accords principaux. Un accord d'acciaccatura ajoutait
//! ses notes à ce compteur, décalant la note liée et perdant la liaison (les
//! 14 liaisons de `ZORBA THE GREEK` après une acciaccatura). La référence est
//! le modèle MuseScore : la note de m2 est la continuation de celle de m1.

use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Accordion</trackName>
  <Instrument id="free-reed.accordion"><longName>Accordion</longName>
    <trackName>Accordion</trackName><instrumentId>free-reed.accordion</instrumentId>
    <Channel><program value="21"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure number="1">
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>eighth</durationType><acciaccatura />
      <Note><pitch>66</pitch><tpc>16</tpc></Note>
      <Note><pitch>70</pitch><tpc>20</tpc></Note>
    </Chord>
    <Chord><durationType>whole</durationType>
      <Note><Tie id="1"></Tie><pitch>67</pitch><tpc>15</tpc></Note>
      <Note><Tie id="2"></Tie><pitch>71</pitch><tpc>19</tpc></Note>
    </Chord>
  </Measure>
  <Measure number="2">
    <Chord><durationType>whole</durationType>
      <Note><endSpanner id="1" /><pitch>67</pitch><tpc>15</tpc></Note>
      <Note><endSpanner id="2" /><pitch>71</pitch><tpc>19</tpc></Note>
    </Chord>
  </Measure>
</Staff>
</Score></museScore>
"#;

fn tie_of(score: &scoretk_core::model::Score, measure: usize, pitch: u8) -> (bool, bool) {
    for segment in &score.measures[measure].segments {
        for (_, item) in segment.iter_elements() {
            if let scoretk_core::model::SegmentItem::ChordRest(chord_rest) = item {
                if let scoretk_core::model::ChordRestKind::Chord(chord) = &chord_rest.kind {
                    for id in &chord.notes {
                        let note = score.note(*id);
                        if note.pitch == pitch {
                            return (note.tie_back.is_some(), note.tie_forward.is_some());
                        }
                    }
                }
            }
        }
    }
    panic!("note {pitch} absente de la mesure {measure}");
}

#[test]
fn une_liaison_apres_une_acciaccatura_est_reliee() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 2.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x");
    assert_eq!(tie_of(&score, 0, 67), (false, true), "départ de la liaison");
    assert_eq!(
        tie_of(&score, 1, 67),
        (true, false),
        "arrivée de la liaison"
    );
    assert_eq!(tie_of(&score, 0, 71), (false, true), "second départ");
    assert_eq!(tie_of(&score, 1, 71), (true, false), "seconde arrivée");
}
