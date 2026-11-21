// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `<Harmony>` porté par un `<FretDiagram>` (lecteur 4.x).

use scoretk_core::model::Annotation;
use scoretk_musescore_import::MsczContainer;

#[test]
fn harmonie_dans_fret_diagram() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/v4/changevisibility.mscx"
    );
    let original = std::fs::read_to_string(path).expect("corpus");
    let injection = "<FretDiagram><frets>4</frets><Harmony><root>15</root></Harmony></FretDiagram>";
    let source = original.replacen("<voice>", &format!("<voice>{injection}"), 1);
    assert_ne!(source, original);
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let harmonies: Vec<_> = score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .flat_map(|segment| &segment.annotations)
        .filter(|entry| matches!(entry.annotation, Annotation::Harmony(_)))
        .collect();
    assert!(!harmonies.is_empty(), "harmonie du FretDiagram perdue");
}
