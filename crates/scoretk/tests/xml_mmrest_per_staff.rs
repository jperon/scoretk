// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Measure::isEmpty(staffIdx)` (`measure.cpp:2286-2330`) et de
// `MMRestLayout::createMMRest` (`mmrestlayout.cpp:161-206`) : un silence
// multi-mesures appartient à une portée, pas à la mesure entière. Les lecteurs
// 3.x/4.x ignoraient le marqueur hors de la portée 0 ; l'export MusicXML
// écrivait alors `<multiple-rest>` sur toutes les parties, ou sur aucune.

const SCORE: &str = r#"<museScore version="3.01"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Haut</trackName>
  <Instrument id="keyboard.piano"><longName>Haut</longName><trackName>Haut</trackName>
    <Channel><program value="1"/></Channel></Instrument></Part>
<Part><Staff id="2"/><trackName>Bas</trackName>
  <Instrument id="keyboard.piano"><longName>Bas</longName><trackName>Bas</trackName>
    <Channel><program value="1"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure number="1"><voice><Rest><durationType>measure</durationType></Rest></voice></Measure>
  <Measure number="2"><voice><Rest><durationType>measure</durationType></Rest></voice></Measure>
</Staff>
<Staff id="2">
  <Measure number="1"><voice><Rest><durationType>measure</durationType></Rest></voice></Measure>
  <Measure number="2"><multiMeasureRest>2</multiMeasureRest></Measure>
</Staff>
</Score></museScore>
"#;

#[test]
fn multi_repos_sur_une_seule_portee() {
    let score = scoretk_musescore_import::read::read_score(
        &scoretk_musescore_import::MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur"),
    )
    .expect("lecture 3.x");
    let xml = String::from_utf8(
        scoretk::write(
            &score,
            scoretk::Format::MusicXml,
            &scoretk::Options::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let parts: Vec<&str> = xml.split("<part id=").skip(1).collect();
    assert_eq!(parts.len(), 2, "deux parties :\n{xml}");
    assert!(!parts[0].starts_with('P'), "découpage inattendu");
    let has = |part: &str| part.contains("<multiple-rest>2</multiple-rest>");
    assert!(
        !has(parts[0]),
        "la portée 1 n'a pas de multi-repos :\n{xml}"
    );
    assert!(has(parts[1]), "la portée 2 porte <multiple-rest>2 :\n{xml}");
}
