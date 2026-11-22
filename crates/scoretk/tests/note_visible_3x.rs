// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.

//! `writeNotehead` (`exportmusicxml.cpp:3910-3913`) écrit
//! `<notehead>none</notehead>` pour une note invisible sur portée normale.
//! Le lecteur 3.x/2.x ne lisait pas `<visible>` sur `<Note>`.

#![cfg(all(feature = "musescore-import", feature = "musicxml-export"))]

use scoretk::{read, write, Format, Options};

#[test]
fn note_invisible_3x_donne_notehead_none() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v3/lyricsline01.mscx");
    let mut mscx = std::fs::read_to_string(path).expect("corpus");
    mscx = mscx.replacen("<Note>", "<Note>\n            <visible>0</visible>", 1);
    let score = read(mscx.as_bytes(), Format::Mscx).expect("lecture");
    assert!(
        score
            .measures
            .iter()
            .any(|m| m
                .segments
                .iter()
                .any(|s| s.iter_elements().any(|(_, item)| match item {
                    scoretk_core::model::SegmentItem::ChordRest(cr) => match &cr.kind {
                        scoretk_core::model::ChordRestKind::Chord(c) =>
                            c.notes.iter().any(|id| !score.note(*id).visible),
                        _ => false,
                    },
                    _ => false,
                }))),
        "le lecteur doit conserver l'invisibilité de la note"
    );
    let xml = write(&score, Format::MusicXml, &Options::default()).expect("export");
    let xml = String::from_utf8(xml).expect("UTF-8");
    assert!(
        xml.contains("<notehead>none</notehead>"),
        "la note invisible doit produire notehead none :\n{xml}"
    );
}
