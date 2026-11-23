// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `TRead::read(FiguredBassItem*)` (`engraving/rw/read400/tread.cpp:1322-1329`) :
// les cinq encadrements `<brackets b0..b4>` sont lus, et
// `FiguredBassItem::startsWithParenthesis` (`engraving/dom/figuredbass.cpp:629-640`)
// en dérive `parentheses="yes"` à l'export MusicXML.

use scoretk_core::model::{Annotation, SegmentType};
use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="3.02"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Continuo</trackName>
  <Instrument id="keyboard.piano"><longName>Continuo</longName>
    <trackName>Continuo</trackName><instrumentId>keyboard.piano</instrumentId>
    <Channel><program value="0"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure><voice>
    <Chord><durationType>whole</durationType>
      <Note><pitch>48</pitch><tpc>14</tpc></Note></Chord>
    <FiguredBass><ticks>1/1</ticks>
      <FiguredBassItem>
        <brackets b0="0" b1="3" b2="4" b3="0" b4="0"/>
        <digit>6</digit>
      </FiguredBassItem>
    </FiguredBass>
  </voice></Measure>
</Staff>
</Score></museScore>
"#;

#[test]
fn crochets_du_chiffrage_sont_lus() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 3.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 3.x");
    let item = score.measures[0]
        .segments
        .iter()
        .filter(|segment| segment.kind == SegmentType::ChordRest)
        .flat_map(|segment| segment.annotations.iter())
        .find_map(|entry| match &entry.annotation {
            Annotation::FiguredBass(bass) => bass.items.first(),
            _ => None,
        })
        .expect("chiffrage lu");
    assert_eq!(item.brackets, [0, 3, 4, 0, 0]);
    assert!(item.starts_with_parenthesis());
}
