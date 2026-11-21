// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Lecture du texte des paroles avec balisage inline (`<i>`, `<b>`…).

use scoretk_core::model::{SegmentItem, SegmentType};
use scoretk_musescore_import::MsczContainer;

#[test]
fn paroles_avec_balisage_inline() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/v4/copypastesymbollist-lyrics.mscx"
    );
    let original = std::fs::read_to_string(path).expect("corpus");
    let source = original.replacen("<text>Nel</text>", "<text><i>Nel</i></text>", 1);
    assert_ne!(source, original, "texte de référence introuvable");
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let mut texts = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.kind != SegmentType::ChordRest {
                continue;
            }
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(chord_rest) = item {
                    for lyric in &chord_rest.lyrics {
                        texts.push(lyric.text.as_str());
                    }
                }
            }
        }
    }
    assert_eq!(
        texts.first().copied(),
        Some("Nel"),
        "paroles avec balisage perdues"
    );
}
