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

//! `\context Staff` au niveau supérieur : une portée, ses voix.
//!
//! `lily/parser.yy` traite `\context Staff` comme `\new Staff` à la création
//! (`context_change`, `lily/context.cc::find_create_context`) : le bloc
//! `\relative << { … } \\ { … } >>` qui suit est le **contenu** de cette
//! portée, donc deux voix. Le lecteur sautait la commande, lisait le `<< >>`
//! comme musique de haut niveau et en faisait une portée par branche (idiome
//! Mutopia), si bien que l'oracle ne pouvait plus fusionner l'unisson : les
//! fichiers `collision-*`/`cluster-*` de `ra` comptaient une note en trop.

use scoretk_core::model::{ChordRestKind, Score, SegmentItem};

fn voix(score: &Score) -> Vec<(usize, u8)> {
    let mut resultat = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    resultat.push((track.voice() as usize, score.note(*id).pitch));
                }
            }
        }
    }
    resultat.sort();
    resultat
}

#[test]
fn contexte_staff_de_haut_niveau_garde_ses_deux_voix() {
    // Source réduite de `input/regression/collision-heads.ly` (sans `\score`).
    let source = "\\version \"2.24.0\"\n\
        \\context Staff \\relative <<\n\
          { c''2 c8 c4. }\n\
          \\\\\n\
          { c8 c4. c2 }\n\
        >>";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1, "une partie");
    assert_eq!(score.staves.len(), 1, "une portée, pas une par branche");
    assert_eq!(
        voix(&score),
        vec![(0, 72), (0, 72), (0, 72), (1, 72), (1, 72), (1, 72)],
        "les deux voix de la portée"
    );
}
