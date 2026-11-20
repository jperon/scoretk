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

//! Champs `L:` insérés dans une ligne de musique continuée par `\`.
//!
//! ABC 2.1 § 2.2.6 (ligne 1529 de l'archive locale) : « any information
//! fields and stylesheet directives are processed at the point where the
//! physical line-break occurs » ; la norme déconseille de pré-processer les
//! continuations en une seule ligne (ligne 349). `abc2xml.py` reconnaît donc
//! chaque champ indépendamment de `mcont` et l'émet comme champ en ligne
//! (lignes 803-818).

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Fraction, Score, StaffId, Track};

/// Hauteurs et durées des chord/rests de la première voix.
fn content(score: &Score) -> (Vec<u8>, Vec<f64>) {
    let track = Track::new(StaffId::from_index(0), 0);
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
fn champ_l_apres_une_continuation() {
    // Réduction de `0626-0635.abc` X:631 : `L:1/16` puis `L:1/8` après un
    // `\`, exactement comme dans l'air réel. Le champ doit changer l'unité
    // à partir de la ligne de musique suivante, et non être avalé comme du
    // texte musical.
    let source = "\
X:1
M:3/4
L:1/8
K:G
G2 A2 B2 |\\
L:1/16
c2 d2 e2 |\\
L:1/8
G2 A2 B2 |]
";
    let score = read_abc(source).expect("lecture des changements d'unité en ligne");
    assert_eq!(score.measures.len(), 3, "trois mesures écrites");
    assert_eq!(
        content(&score),
        (
            vec![67, 69, 71, 72, 74, 76, 67, 69, 71],
            vec![0.25, 0.25, 0.25, 0.125, 0.125, 0.125, 0.25, 0.25, 0.25]
        ),
        "hauteurs, durées écrites et changements d'unité aux bons endroits"
    );

    // Cas « non concerné » : une continuation entre deux lignes de musique
    // sans champ doit toujours être jointe, sans mesure supplémentaire.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nC D E F |\\\nG A B c |]\n";
    let score = read_abc(source).expect("continuation musicale ordinaire");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        content(&score).0,
        vec![60, 62, 64, 65, 67, 69, 71, 72],
        "la continuation musicale reste inchangée"
    );
}

#[test]
fn champ_apres_continuation_en_fin_de_fichier() {
    // Cas limite : le champ suit le `\` mais aucune musique ne vient après.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nC2 |\\\nL:1/16\n";
    let score = read_abc(source).expect("champ final après continuation");
    assert_eq!(
        score.measures[0].len,
        Fraction::new(1, 4),
        "la mesure avant le champ garde sa longueur écrite"
    );
    assert_eq!(
        score.notes.iter().map(|note| note.pitch).collect::<Vec<_>>(),
        vec![60],
        "aucune note ajoutée par le champ final"
    );
}
