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

//! Une ligne de musique peut commencer par un champ en ligne `[V:…]`
//! (ABC 2.1 §7.3). Réduit de `parsing.test__abc3.abc` X:360 (`[V:1]f|`) :
//! sans `K:`, le garde-fou « ligne de prose » prenait le `V` pour du texte
//! et l'air entier était lu sans notes, alors qu'abc2xml.py lui donne
//! l'armure C par défaut.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{ChordRestKind, Score, SegmentItem};

fn pitches(source: &str) -> Vec<u8> {
    let score: Score = read_abc(source).expect("lecture ABC");
    score
        .measures
        .iter()
        .flat_map(|m| m.segments.iter())
        .flat_map(|s| s.iter_elements())
        .filter_map(|(_, item)| match item {
            SegmentItem::ChordRest(cr) => match &cr.kind {
                ChordRestKind::Chord(ch) => Some(
                    ch.notes
                        .iter()
                        .map(|id| score.note(*id).pitch)
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn champ_en_ligne_sans_k() {
    let source = "X: 360\n[V:1]f|\n";
    assert_eq!(pitches(source), vec![77], "air vide :\n{source}");
}

#[test]
fn champ_en_ligne_avec_k() {
    let source = "X:360\nK:C\n[V:1]f|\n";
    assert_eq!(pitches(source), vec![77], "air vide :\n{source}");
}
