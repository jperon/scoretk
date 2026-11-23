// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Read206::readMeasure` (`engraving/rw/read206/read206.cpp:2863-2905`) :
// une clef au tick zéro devient la clef par défaut de la portée et disparaît ;
// une clef de début de mesure est un segment `Clef` (jamais `HeaderClef`),
// marqué « après la barre de mesure ». `writeElement`
// (`exportmusicxml.cpp:7986-8007`) en tire `after-barline="yes"`.

use scoretk_core::model::{ClefType, SegmentItem, SegmentType};
use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Piano</trackName>
  <Instrument id="keyboard.piano"><longName>Piano</longName>
    <trackName>Piano</trackName><instrumentId>keyboard.piano</instrumentId>
    <Channel><program value="0"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure>
    <Clef><concertClefType>G8vb</concertClefType>
      <transposingClefType>G8vb</transposingClefType></Clef>
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Rest><durationType>measure</durationType><duration>4/4</duration></Rest>
  </Measure>
  <Measure>
    <Clef><concertClefType>F</concertClefType>
      <transposingClefType>F</transposingClefType></Clef>
    <Rest><durationType>measure</durationType><duration>4/4</duration></Rest>
  </Measure>
</Staff>
</Score></museScore>
"#;

fn clefs(score: &scoretk_core::model::Score, measure: usize) -> Vec<(SegmentType, ClefType)> {
    score.measures[measure]
        .segments
        .iter()
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(move |(_, item)| match item {
                    SegmentItem::Clef(clef) => Some((segment.kind, clef.concert)),
                    _ => None,
                })
        })
        .collect()
}

#[test]
fn clef_2x_de_debut_de_mesure_reste_une_clef_ordinaire() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 2.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 2.x");
    // Mesure 0 : la clef du tick zéro est la clef par défaut (HeaderClef).
    assert_eq!(
        clefs(&score, 0),
        vec![(SegmentType::HeaderClef, ClefType::G8Vb)]
    );
    // Mesure 1 : `Clef`, pas `HeaderClef` (l'export l'écrit after-barline).
    assert_eq!(clefs(&score, 1), vec![(SegmentType::Clef, ClefType::F)]);
}
