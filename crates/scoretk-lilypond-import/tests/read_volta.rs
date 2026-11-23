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

//! `\volta` : sélection par passage à l'intérieur d'un `\repeat unfold`, et
//! alternatives imbriquées.
//!
//! Réductions de `input/regression/volta-spec-*.ly`,
//! `volta-bracket-nest.ly`. `\volta` appartient à la reprise **la plus
//! intérieure** (`lily/unfolded-repeat-iterator.cc`,
//! `lily/alternative-sequence-iterator.cc`) : une reprise non dépliée garde
//! toutes ses fins dans le modèle gravé.

use scoretk_core::model::{ChordRestKind, SegmentItem};

fn notes(score: &scoretk_core::model::Score) -> Vec<u8> {
    let mut resultat = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_track, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    resultat.push(score.note(*id).pitch);
                }
            }
        }
    }
    resultat
}

#[test]
fn volta_selectionne_le_passage_dans_repeat_unfold() {
    // `\repeat unfold 2` joue le corps deux fois ; `\volta 1` ne sonne qu'au
    // premier passage, `\volta 2` au second, `\volta 3` (hors plage) jamais.
    let source = "\\version \"2.24.0\"\n\
        { \\repeat unfold 2 << \\volta 1 c'4 \\volta 2 e'4 \\volta 3 g'4 >> }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(notes(&score), vec![60, 64]);
}

#[test]
fn alternative_imbriquee_garde_ses_fins() {
    // `volta-bracket-nest` : les alternatives de la portée d'un
    // `\repeat volta` empilent leurs crochets ; toutes leurs notes restent
    // dans le modèle gravé (d, e, f, g, a, b).
    let source = "\\version \"2.24.0\"\n\
        \\fixed c' {\n\
          \\repeat volta 6 { d1 \\alternative {\n\
            \\volta 1 e1\n\
            \\volta 2,3,4,5,6 { f1 \\alternative { \\volta 2,3,4,5 g1 \\volta 6 a1 } }\n\
          } }\n\
          b1\n\
        }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(notes(&score), vec![62, 64, 65, 67, 69, 71]);
}
