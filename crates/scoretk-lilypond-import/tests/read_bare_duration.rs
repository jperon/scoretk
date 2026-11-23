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

//! `\longa`, `\breve`, `\maxima` employés seuls : durées nues.
//!
//! `lily/parser.yy` `event_chord: duration` et
//! `scm/music-functions.scm::expand-repeat-notes!` : une durée sans hauteur
//! répète l'accord précédent. Réduction de `input/regression/monochords.ly`.

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
fn breve_et_longa_seuls_reprennent_l_accord_precedent() {
    // `\longa \breve 1 2 4 8 8` après un accord : durées nues, elles répètent
    // l'accord (`lily/parser.yy` `event_chord: duration`,
    // `expand-repeat-notes!`) et non des silences. `monochords` : 7 attaques
    // par groupe dans la référence MIDI de LilyPond.
    let source = "\\version \"2.24.0\"\n\
        \\relative { <a' a>4 \\breve 1 }";
    let score = scoretk_lilypond_import::read_lilypond(source).expect("lecture");
    let mut attaques: Vec<_> = score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .filter(|segment| {
            matches!(
                segment.iter_elements().next().map(|(_, item)| item),
                Some(SegmentItem::ChordRest(chord_rest))
                    if matches!(chord_rest.kind, ChordRestKind::Chord(_))
            )
        })
        .map(|segment| segment.tick)
        .collect();
    attaques.dedup();
    assert!(
        attaques.len() >= 3,
        "trois attaques (l'accord, puis \\breve et 1) : {attaques:?}"
    );
    assert!(notes(&score).iter().all(|pitch| *pitch == 69));
}
