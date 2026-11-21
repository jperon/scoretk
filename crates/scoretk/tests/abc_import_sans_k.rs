// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A tune without `K:` is non-conforming (ABC 2.1 §3.1 requires it) but
//! `abc2xml.py:2423` defaults it to `K:C treble` and reads it anyway. The
//! reader rejected such tunes with `en-tête ABC incomplet : K:`, so scoretk
//! produced no MusicXML where the alternative did.
//! Accept with the already-initialised C major / 4/4 / treble defaults.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn air_sans_champ_k() {
    let source = "X:1\nT:No key field\nM:4/4\nL:1/4\nC D E F | G A B c |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture sans K:");
    assert_eq!(score.notes.len(), 8);
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert!(
        xml.contains("<fifths>0</fifths>"),
        "C major default:\n{xml}"
    );
}
