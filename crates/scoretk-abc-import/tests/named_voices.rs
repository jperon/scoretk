// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. L'air est réduit de `flattener.test__abcNoChordVoice.abc`.
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

//! Noms de voix dans `V:` et `%%score`.
//!
//! Port de `abc2xml.py` 269, `splitHeaderVoices` (ligne 779) et
//! `voiceNamesAndMaps` (ligne 2253) : `%%score (S A) (T B)` regroupe deux
//! voix nommées sur une même portée. Un nom n'est pas un numéro : toutes les
//! voix nommées ne doivent pas être réduites à la voix 1.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Score, StaffId, Track};

const QUATRE_VOIX: &str = "\
X:1
M:4/4
%%score (S A) (T B)
V:S clef=treble
V:A clef=treble
V:T clef=bass
V:B clef=bass
L:1/4
K:C
V:S
|e4 |
V:A
|E4 |
V:T
|B,4 |
V:B
|E,4 |
";

/// Hauteurs d'une (portée, voix) du modèle.
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
fn voix_nommees_regroupees_par_score() {
    let score = read_abc(QUATRE_VOIX).expect("lecture");
    assert_eq!(score.parts.len(), 2, "deux parties : (S A) et (T B)");
    assert_eq!(
        score.measures.len(),
        1,
        "une mesure globale portant quatre voix"
    );
    assert!(
        score.parts.iter().all(|part| part.staves.len() == 1),
        "une portée par partie"
    );
    // Les quatre voix restent distinctes, deux par portée.
    let bas = score.parts[1].staves[0];
    assert_eq!(pitches(&score, 0, 0), vec![76], "S : e4");
    assert_eq!(pitches(&score, 0, 1), vec![64], "A : E4");
    assert_eq!(pitches(&score, bas.index(), 0), vec![59], "T : B,4");
    assert_eq!(pitches(&score, bas.index(), 1), vec![52], "B : E,4");
}

#[test]
fn numeros_et_noms_sans_score_restent_distincts() {
    // Cas « non concerné » : les numéros ABC gardent leur regroupement.
    let source = "\
X:1
M:4/4
%%score (1 2) 3
L:1/4
K:C
V:1
|C |
V:2
|E |
V:3
|G |
";
    let score = read_abc(source).expect("lecture des numéros");
    assert_eq!(score.parts.len(), 2);
    assert_eq!(pitches(&score, 0, 0), vec![60]);
    assert_eq!(pitches(&score, 0, 1), vec![64]);
    assert_eq!(pitches(&score, 1, 0), vec![67]);

    // Cas limite : voix nommées sans `%%score` : une portée par voix.
    let source = QUATRE_VOIX.replace("%%score (S A) (T B)\n", "");
    let score = read_abc(&source).expect("lecture des noms sans score");
    assert_eq!(score.parts.len(), 4);
    assert_eq!(
        score.measures.len(),
        1,
        "une mesure globale portant quatre voix"
    );
}
