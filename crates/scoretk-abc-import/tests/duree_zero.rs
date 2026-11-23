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

//! Une durée explicite `0` n'est pas définie par ABC 2.1 §4.3. L'alternative
//! `abc2xml.py::abcdur` (ligne 502) la traite comme une durée absente
//! (`return ''`), donc comme une unité `L:` par défaut. Réduit de
//! `note.test__abcZeroLength.abc` (`C0 D1 [EG]0 [FA]1`), rejeté par le lecteur.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{ChordRestKind, Score, SegmentItem};

fn notes(source: &str) -> Vec<(u8, scoretk_core::model::Fraction)> {
    let score: Score = read_abc(source).expect("lecture ABC");
    score
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .filter_map(|s| {
            s.iter_elements().find_map(|(_, item)| match item {
                SegmentItem::ChordRest(cr) => match &cr.kind {
                    ChordRestKind::Chord(ch) => ch
                        .notes
                        .iter()
                        .map(|id| (score.note(*id).pitch, s.tick))
                        .next(),
                    _ => None,
                },
                _ => None,
            })
        })
        .collect()
}

#[test]
fn duree_zero_vaut_unite_par_defaut() {
    let source = "X:1\nL:1/8\nC0 D1 [EG]0 [FA]1\n";
    let got = notes(source);
    let expected = notes("X:1\nL:1/8\nC D [EG] [FA]\n");
    assert_eq!(got, expected, "air rejeté ou durée fausse :\n{source}");
}
