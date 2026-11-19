// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. L'air est réduit de `flattener.test__abcOverlay.abc`.
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

//! Superpositions `&` : les notes après le `&` attaquent en même temps que
//! celles qui le précèdent, sur une autre voix de la même portée.
//!
//! Port de `abc2xml.py` 269, `voice_overlay` (ligne 425) et
//! `mergePartMeasure` (ligne 951) : l'export écrit alors un `<backup>`.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Score, StaffId, Track};

fn pitches(score: &Score, staff: usize, voice: u8) -> Vec<u8> {
    let track = Track::new(StaffId::from_index(staff), voice);
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(track))
        .flat_map(|chord_rest| {
            chord_rest
                .chord_ref()
                .map(|chord| {
                    chord
                        .notes
                        .iter()
                        .map(|id| score.note(*id).pitch)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn overlays_sont_des_voix_simultanees() {
    let source = "\
X:1
M:4/4
L:1/4
K:C
C4 | G4 & E4 | B4 & d4 & f4 |
";
    let score = read_abc(source).expect("lecture des superpositions");
    // Une seule partie, une seule portée, trois voix pour les superpositions.
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.parts[0].staves.len(), 1);
    assert_eq!(score.measures.len(), 3, "trois mesures écrites");
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![60, 67, 64, 71, 74, 77],
        "toutes les notes sont lues dans leur ordre d'écriture"
    );
    // `G4` et `E4` attaquent ensemble (voix 0 et 1 de la mesure 2), puis
    // `B4`, `d4` et `f4` attaquent ensemble dans la mesure 3.
    assert_eq!(pitches(&score, 0, 0), vec![60, 67, 71]);
    assert_eq!(pitches(&score, 0, 1), vec![64, 74]);
    assert_eq!(pitches(&score, 0, 2), vec![77]);
}

#[test]
fn superposition_apres_anacrouse() {
    // Réduit de `flattener.test__abcPickupOverlay.abc` : l'anacrouse d'un
    // croche ne doit pas être étirée par les mesures vides des voix créées
    // plus tard pour les superpositions.
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nB, | C4 | D4 | G4 & E4 |\n";
    let score = read_abc(source).expect("lecture de l'anacrouse");
    assert_eq!(score.measures.len(), 4);
    assert_eq!(
        score.measures[0].len,
        scoretk_core::model::Fraction::new(1, 4),
        "l'anacrouse reste courte malgré les voix superposées suivantes"
    );
    assert_eq!(pitches(&score, 0, 0), vec![59, 60, 62, 67]);
    assert_eq!(pitches(&score, 0, 1), vec![64]);
}

#[test]
fn musique_sans_overlay_et_controle() {
    // Cas « non concerné » : sans `&`, une seule voix et les mesures se
    // succèdent exactement comme avant le port.
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nC4 | D4 | E4 |]\n";
    let score = read_abc(source).expect("lecture sans superposition");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.measures.len(), 3);
    assert_eq!(pitches(&score, 0, 0), vec![60, 62, 64]);
}
