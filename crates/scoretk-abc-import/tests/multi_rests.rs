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

//! Pauses de plusieurs mesures `Znn` / `Xnn`.
//!
//! Port de `abc2xml.py` 269, `fixSlurs.f` (lignes 767-775) : `Z24` est
//! remplacé par 24 pauses de mesure `Z|Z|…|Z`. Le nombre est donc une
//! **répétition de mesures**, et non une pause longue dans une seule mesure.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Fraction, StaffId, Track};

/// Nombre de mesures, de notes, de pauses de mesure et longueur nominale.
fn content(source: &str) -> (usize, Vec<u8>, usize) {
    let score = read_abc(source).expect("lecture");
    let track = Track::new(StaffId::from_index(0), 0);
    let rests = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(track))
        .filter(|chord_rest| chord_rest.is_full_measure_rest())
        .count();
    (
        score.measures.len(),
        score.notes.iter().map(|n| n.pitch).collect(),
        rests,
    )
}

#[test]
fn z_et_x_se_repetent_sur_leurs_mesures() {
    // `Z24` : 24 mesures, une pause de mesure chacune, aucune note.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nZ24|]\n";
    assert_eq!(
        content(source),
        (24, vec![], 24),
        "Z24 vaut 24 mesures pleines"
    );

    // Cas limite : un nombre nul est traité comme une seule pause.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nZ0|]\n";
    assert_eq!(content(source), (1, vec![], 1), "Z0 : une pause au minimum");
}

#[test]
fn pauses_au_milieu_dun_air_ordinaire() {
    // Musique ordinaire avant et après : les mesures de pause sont insérées
    // sans décaler les hauteurs, les durées ni les barres.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nC D | Z3 | E F |]\n";
    let score = read_abc(source).expect("lecture");
    assert_eq!(
        score.measures.len(),
        5,
        "1 mesure de musique + 3 pauses + 1 mesure"
    );
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![60, 62, 64, 65]
    );
    for measure in &score.measures {
        assert_eq!(measure.nominal_len, Fraction::ONE);
    }
    assert_eq!(content(source).2, 3, "trois pauses de mesure");

    // Cas « non concerné » : `Z` seul et une pause écrite `z4` ne changent
    // pas : une mesure, une pause (de mesure pour `Z`, de noire pour `z4`).
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nZ |]\n";
    assert_eq!(content(source), (1, vec![], 1));
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nz4 |]\n";
    assert_eq!(content(source).0, 1);
    assert_eq!(content(source).2, 0, "z4 n'est pas une pause de mesure");
}
