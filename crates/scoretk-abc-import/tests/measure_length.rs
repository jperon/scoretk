// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. L'air réduit provient du corpus `abcjs-tests` (voir
// `docs/agents/abc.md` § 16.3/§ 16.4).
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

//! Longueur ÉCRITE des mesures (port de `abc2xml.py::doMaat`, ligne 674).
//!
//! `abc2xml.py` n'étire jamais une mesure courte jusqu'à la longueur nominale
//! de `M:` : il écrit les notes et barres telles qu'elles sont notées. Le
//! modèle pivot doit donc porter la longueur écrite, et non `actual.max(nominal)`.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Fraction, Score, StaffId, Track};

/// X:5 de `abcjs-tests/extracted-abc/tablature.test__bracketPlacement.abc` :
/// deux voix, une seule mesure courte chacune, sans barre fermante.
const TABLATURE_X5: &str = "\
X:5
L:1/8
%%staves {RH LH}
V: RH clef=treble
V: LH clef=bass
K:C
[V: RH]
|: c
[V: LH]
|: G,
";

/// Hauteurs et durées des chord/rests d'une voix, dans l'ordre du fichier.
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
fn mesure_courte_garde_sa_longueur_ecrite() {
    let score = read_abc(TABLATURE_X5).expect("lecture de l'air réduit");

    // Les noms de voix `RH`/`LH` ne sont pas des numéros ABC : le lecteur les
    // ramène à une seule voix, qui enchaîne donc deux mesures d'un croche.
    assert_eq!(score.measures.len(), 2, "deux mesures écrites");
    // Première mesure : anacrouse, déjà courte avant le port.
    assert_eq!(score.measures[0].len, Fraction::new(1, 8));
    // C'est le point du port : la seconde mesure, non-anacrouse, gardait
    // `actual.max(nominal)` = 4/4 (1.0) au lieu de sa longueur écrite.
    assert_eq!(
        score.measures[1].len,
        Fraction::new(1, 8),
        "la mesure doit garder sa longueur écrite (1/8), pas la nominale"
    );
    for measure in &score.measures {
        assert_eq!(measure.nominal_len, Fraction::ONE, " mesure nominale 4/4");
    }
    assert_eq!(
        score.measures[1].start_tick,
        scoretk_core::model::Tick::new(1, 8)
    );
    assert_eq!(
        voice_content(&score, 0, 0),
        (vec![72, 55], vec![0.125, 0.125])
    );
    assert_eq!(score.notes.len(), 2);
}

#[test]
fn anacrouse_et_mesures_pleines_inchangees() {
    // Cas « non concerné » : l'anacrouse est déjà courte dans l'ancien lecteur
    // et la mesure pleine reste pleine. Le port ne doit rien y changer.
    let source = "\
X:1
M:4/4
L:1/8
K:C
e2 | C D E F G A B c |
";
    let score = read_abc(source).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].len, Fraction::new(1, 4), "anacrouse");
    assert_eq!(score.measures[1].len, Fraction::ONE, "mesure pleine");
    assert_eq!(
        voice_content(&score, 0, 0).0,
        vec![76, 60, 62, 64, 65, 67, 69, 71, 72]
    );
}

#[test]
fn limites_fin_de_fichier_et_mesure_vide() {
    // Fin de fichier sans barre fermante : la mesure reste à sa longueur
    // écrite (4 croches = 1/2), pas à la longueur nominale.
    let eof = "\
X:1
M:4/4
L:1/8
K:C
C D E F
";
    let score = read_abc(eof).expect("lecture sans barre finale");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::new(1, 2));
    assert_eq!(voice_content(&score, 0, 0).1, vec![0.125; 4]);

    // Mesure vide (aucune note) : repli sur la longueur nominale pour que le
    // modèle reste valide, comme avant le port.
    let empty = "\
X:1
M:3/4
L:1/8
K:C
";
    let score = read_abc(empty).expect("lecture de l'air vide");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::new(3, 4));
    assert!(
        voice_content(&score, 0, 0).0.is_empty(),
        "aucune note écrite"
    );
}
