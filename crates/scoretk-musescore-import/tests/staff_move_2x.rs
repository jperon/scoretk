// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Read206::readChordRestProperties206`
// (`engraving/rw/read206/read206.cpp:1821-1822`) : le champ 2.x `<move>` est
// l'ancien nom de `staffMove`. MuseScore 4.7.4 l'exporte en `<staff>2</staff>`
// (`exportmusicxml.cpp:4469`), et les importe via `determineStaffMoveVoice`
// (`importmusicxmlpass1.cpp:578-624`, `importmusicxmlpass2.cpp:7263-7270`).

use scoretk_core::model::SegmentItem;
use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><Staff id="2"/><trackName>Piano</trackName>
  <Instrument id="keyboard.piano"><longName>Piano</longName>
    <trackName>Piano</trackName><instrumentId>keyboard.piano</instrumentId>
    <Channel><program value="1"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure number="1">
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Chord><move>1</move><durationType>whole</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc></Note>
    </Chord>
  </Measure>
</Staff>
<Staff id="2">
  <Measure number="1"><Rest><durationType>measure</durationType></Rest></Measure>
</Staff>
</Score></museScore>
"#;

#[test]
fn move_2x_devient_staff_move() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 2.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x");
    let moves: Vec<i8> = score
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .flat_map(|segment| segment.iter_elements())
        .filter_map(|(_, item)| match item {
            SegmentItem::ChordRest(cr) => cr.chord_ref().map(|chord| chord.staff_move),
            _ => None,
        })
        .collect();
    assert_eq!(moves, vec![1], "<move>1</move> doit devenir staffMove = 1");
}
