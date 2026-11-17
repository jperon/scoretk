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

//! Test de fumée de la lecture MIDI : un fichier du corpus doit se relire.

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
    assert!(!score.measures.is_empty(), "aucune mesure");
    assert!(!score.notes.is_empty(), "aucune note");
    for measure in &score.measures {
        eprintln!(
            "mesure {} tick {:?} len {:?} nominal {:?} segments {}",
            measure.number,
            measure.start_tick,
            measure.len,
            measure.nominal_len,
            measure.segments.len()
        );
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let mscz2xml::model::SegmentItem::TimeSig(ts) = item {
                    eprintln!(
                        "  sig {} {:?} {}",
                        segment.kind as u8, segment.tick, ts.numerator
                    );
                }
            }
        }
    }
    let xml = mscz2xml::export::export_musicxml(&score).expect("export");
    std::fs::write(
        std::env::temp_dir().join("t42_import_dynamics.musicxml"),
        &xml,
    )
    .ok();
    eprintln!("{}", &xml[..xml.len().min(6000)]);
}

#[test]
fn lit_tuplets() {
    let bytes = std::fs::read(corpus("v3/testTuplets8.ref.mid")).expect("corpus MIDI");
    let score = scoretk_midi::read_midi(&bytes).expect("lecture MIDI");
    let xml = mscz2xml::export::export_musicxml(&score).expect("export");
    std::fs::write(
        std::env::temp_dir().join("t42_import_tuplets.musicxml"),
        &xml,
    )
    .ok();
    eprintln!("{}", &xml[..xml.len().min(6000)]);
}
