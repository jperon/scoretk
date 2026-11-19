// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. Les airs sont écrits à la main pour l'occasion.
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

//! Transposition ABC (`transpose=` dans `K:`/`V:`, `%%MIDI transpose`).
//!
//! Comme `abc2xml.py`, la lecture garde les hauteurs **écrites** dans le
//! MusicXML et déclare l'écart dans un élément `<transpose>` ; le modèle
//! pivot porte lui les hauteurs **réelles** (écrites + transposition).

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Score, StaffId, Track};

/// Hauteurs et durées d'une voix d'une portée, dans l'ordre du fichier.
fn voice_content(score: &Score, staff: usize, voice: u8) -> (Vec<u8>, Vec<f64>) {
    let track = Track::new(StaffId::from_index(staff), voice);
    let mut pitches = Vec::new();
    let mut durations = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(chord_rest) = segment.chord_rest(track) {
                if let Some(chord) = chord_rest.chord_ref() {
                    pitches.extend(chord.notes.iter().map(|id| score.note(*id).pitch));
                }
                durations.push(chord_rest.duration.ticks().as_f64());
            }
        }
    }
    (pitches, durations)
}

#[test]
fn transposition_k_et_v() {
    let source = "\
X:1
T:Transposition
M:4/4
L:1/8
K:C transpose=2
V:1
C D E F | [GB]2 z2 | c2-d2 |]
V:2 transpose=-2
C D E F | [GB]2 z2 | c2-d2 |]
";
    let score = read_abc(source).expect("lecture");

    assert_eq!(score.measures.len(), 3, "trois mesures dans chaque voix");
    // Voix 1 : transposition +2, hauteurs écrites C D E F, [G B], c-d.
    assert_eq!(
        voice_content(&score, 0, 0),
        (
            vec![62, 64, 66, 67, 69, 73, 74, 76],
            vec![0.125, 0.125, 0.125, 0.125, 0.25, 0.25, 0.25, 0.25]
        )
    );
    // Les deux voix sont lues, chacune avec sa propre transposition.
    assert_eq!(score.notes.len(), 16);
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![
            62, 64, 66, 67, 69, 73, 74, 76, // voix 1, +2
            58, 60, 62, 63, 65, 69, 70, 72, // voix 2, -2
        ]
    );
    assert_eq!(
        score
            .parts
            .iter()
            .map(|part| part.instrument.transpose.map(|value| value.chromatic))
            .collect::<Vec<_>>(),
        vec![Some(2), Some(-2)],
        "chaque partie porte sa transposition"
    );

    // Cas « non concerné » : sans transpose=, la musique est inchangée.
    let plain = source
        .replace(" transpose=2", "")
        .replace(" transpose=-2", "");
    let score_plain = read_abc(&plain).expect("lecture");
    assert_eq!(
        score_plain
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![
            60, 62, 64, 65, 67, 71, 72, 74, //
            60, 62, 64, 65, 67, 71, 72, 74,
        ]
    );
    assert!(score_plain
        .parts
        .iter()
        .all(|part| part.instrument.transpose.is_none()));
}

#[test]
fn transposition_midi_directive() {
    let source = "\
X:1
T:MidiTranspose
M:4/4
L:1/8
%%MIDI transpose -2
K:A
A B c d | [ce]2 z2 |]
";
    let score = read_abc(source).expect("lecture");

    assert_eq!(score.measures.len(), 2);
    // En K:A, `c` est do dièse : hauteurs écrites 69 71 73 74, [73 76].
    assert_eq!(
        voice_content(&score, 0, 0),
        (
            vec![67, 69, 71, 72, 71, 74],
            vec![0.125, 0.125, 0.125, 0.125, 0.25, 0.25]
        )
    );
    assert_eq!(
        score.parts[0]
            .instrument
            .transpose
            .map(|value| value.chromatic),
        Some(-2)
    );

    // Sans la directive, les hauteurs écrites restent concert.
    let plain = source.replace("%%MIDI transpose -2\n", "");
    let score_plain = read_abc(&plain).expect("lecture");
    assert_eq!(
        voice_content(&score_plain, 0, 0).0,
        vec![69, 71, 73, 74, 73, 76]
    );
    assert!(score_plain.parts[0].instrument.transpose.is_none());

    // Cas limite : valeur invalide, avertissement et aucun décalage.
    let invalid = source.replace("%%MIDI transpose -2", "%%MIDI transpose abc");
    let score_invalid = read_abc(&invalid).expect("lecture");
    assert_eq!(
        voice_content(&score_invalid, 0, 0).0,
        vec![69, 71, 73, 74, 73, 76]
    );
    assert!(score_invalid.parts[0].instrument.transpose.is_none());

    // Cas limite : `transpose=0` n'écrit aucun élément `<transpose>`.
    let zero = source
        .replace("%%MIDI transpose -2\n", "")
        .replace("K:A", "K:A transpose=0");
    let score_zero = read_abc(&zero).expect("lecture");
    assert_eq!(
        voice_content(&score_zero, 0, 0).0,
        vec![69, 71, 73, 74, 73, 76]
    );
    assert!(score_zero.parts[0].instrument.transpose.is_none());
}
