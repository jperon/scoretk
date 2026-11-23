// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Staff::show()` (`engraving/dom/staff.cpp:1420-1423`) et
// `TRead::read(Part*)` (`engraving/rw/read500/tread.cpp`) : la visibilité
// d'une partie est portée par `<show>` et masque toutes ses portées, même
// sans `<isStaffVisible>`. `MeasureLayout::validMMRestMeasure`
// (`engraving/rendering/score/measurelayout.cpp:590-620`) ignore ces portées
// pour le regroupement des silences multi-mesures.

use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="3.02"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Chœur</trackName>
  <Instrument id="voice.tenor"><longName>Tenor</longName>
    <trackName>Tenor</trackName><instrumentId>voice.tenor</instrumentId>
    <Channel><program value="52"/></Channel></Instrument></Part>
<Part><Staff id="2"/><show>0</show><trackName>Klaver</trackName>
  <Instrument id="keyboard.piano"><longName>Piano</longName>
    <trackName>Klaver</trackName><instrumentId>keyboard.piano</instrumentId>
    <Channel><program value="0"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure><voice><Rest><durationType>measure</durationType></Rest></voice></Measure>
</Staff>
<Staff id="2">
  <Measure><voice><Chord><durationType>whole</durationType>
    <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice></Measure>
</Staff>
</Score></museScore>
"#;

#[test]
fn show_zero_masque_la_partie() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 3.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 3.x");
    let visibilites: Vec<bool> = score.parts.iter().map(|part| part.visible).collect();
    assert_eq!(visibilites, vec![true, false]);
    // Les portées restent individuellement visibles : c'est `Part::show` qui
    // les masque (`Staff::show()` compose les deux).
    assert!(score.staves.iter().all(|staff| staff.visible));
}

#[test]
fn show_absent_vaut_visible() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 3.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 3.x");
    assert!(score.parts[0].visible);
}
