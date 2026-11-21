// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! A fermata (`H` or `!fermata!`) before a rest must survive the import.
//! `abc2xml.py::doDecos` (lines 1491-1492) writes `<fermata>` on the rest
//! note; ABC 2.1 §4.14 defines `H` as the fermata symbol. The reader dropped
//! every articulation/ornament whose event was a rest because it only pushed
//! them onto `Chord`, while the MusicXML exporter already emits rest fermatas
//! from a segment `Annotation::Fermata` (`notations.rs::rest_notations`).

#![cfg(all(feature = "abc-import", feature = "musicxml-export"))]

#[test]
fn fermata_sur_silence() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC D Hz E | Hc z G F |]\n";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    // One fermata on `Hz`, one on `Hc`: neither may be lost.
    assert_eq!(
        xml.matches("<fermata").count(),
        2,
        "fermata on rest lost:\n{xml}"
    );
}
