// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `TRead::read(FretDiagram*)`
// (`engraving/rw/read460/tread.cpp:806-870`) : la grille (`<fretDiagram>`) est
// lue avec son `<Harmony>` et s'exporte en `<frame>`
// (`exportmusicxml.cpp:9049-9122`).

use scoretk_core::model::{Annotation, FretDotType, FretMarkerType, SegmentType};
use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="4.60"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Guitare</trackName>
  <Instrument id="pluck.guitar"><longName>Guitare</longName>
    <trackName>Guitare</trackName><instrumentId>pluck.guitar</instrumentId>
    <Channel><program value="24"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure><voice>
    <Chord><durationType>whole</durationType>
      <Note><pitch>67</pitch><tpc>15</tpc></Note></Chord>
    <FretDiagram>
      <frets>4</frets>
      <Harmony><harmonyInfo><name>G</name><root>15</root></harmonyInfo>
        <name>G</name><root>15</root></Harmony>
      <fretDiagram>
        <string no="0"><dot fret="3">normal</dot></string>
        <string no="2"><marker>circle</marker></string>
        <string no="5"><dot fret="1">cross</dot></string>
        <barre start="0" end="-1">3</barre>
        </fretDiagram>
      </FretDiagram>
  </voice></Measure>
</Staff>
</Score></museScore>
"#;

#[test]
fn grille_d_accord_4x_est_lue() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 4.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 4.x");
    // La note et le FretDiagram sont deux annotations du même segment.
    let mut diagram = None;
    for segment in &score.measures[0].segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        for entry in &segment.annotations {
            if let Annotation::Harmony(harmony) = &entry.annotation {
                diagram = harmony.fret_diagram.clone();
            }
        }
    }
    let diagram = diagram.expect("grille lue");
    assert_eq!((diagram.strings, diagram.frets), (6, 4));
    assert_eq!(diagram.dots.len(), 2);
    assert_eq!((diagram.dots[0].string, diagram.dots[0].fret), (0, 3));
    assert_eq!(diagram.dots[1].kind, FretDotType::Cross);
    assert_eq!(diagram.markers[0].kind, FretMarkerType::Circle);
    assert_eq!(diagram.barres[0].fret, 3);
    assert_eq!(diagram.barres[0].end, -1);
}
