// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Every `K:` field is written as a `<key>` element by `abc2xml.py` (even
//! when the key is unchanged), so a repeated identical `K:` must survive.
//! ABC 2.1 §3.2 "Repeated information fields" treats it as a replacement;
//! the writer's `keysig_timesig` emits one `<key>` per `SegmentItem::KeySig`.
//! The reader only scheduled changes, dropping redundant fields.

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn key_identique_repetee() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D E F |\nK:C\nG A B c |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    assert_eq!(
        xml.matches("<key").count(),
        2,
        "repeated K: field lost:\n{xml}"
    );
}
