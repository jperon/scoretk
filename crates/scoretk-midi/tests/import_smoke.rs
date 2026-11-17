// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Tests de fumée de la lecture MIDI : un fichier du corpus doit se relire et
//! s'exporter en MusicXML.

use std::path::PathBuf;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/midi")
        .join(name)
}

#[test]
fn lit_dynamics() {
    let bytes = std::fs::read(corpus("v4/dynamics.ref.mid")).expect("corpus MIDI");
    let score = scoretk_midi::read_midi(&bytes).expect("lecture MIDI");
    assert!(!score.parts.is_empty(), "aucune partie");
    assert_eq!(score.measures.len(), 5, "mesures");
    assert_eq!(score.notes.len(), 20, "notes");
    assert!(score.notes.iter().all(|note| note.velocity.is_some()));
    let xml = scoretk_core::export::export_musicxml(&score).expect("export");
    assert!(xml.contains("<score-partwise"), "{xml}");
    assert!(xml.contains("<step>D</step>"), "{xml}");
}

#[test]
fn lit_tuplets_et_voix() {
    let bytes = std::fs::read(corpus("v3/testTuplets8.ref.mid")).expect("corpus MIDI");
    let score = scoretk_midi::read_midi(&bytes).expect("lecture MIDI");
    assert!(!score.notes.is_empty(), "aucune note");
    let xml = scoretk_core::export::export_musicxml(&score).expect("export");
    assert!(xml.contains("<score-partwise"), "{xml}");
}
