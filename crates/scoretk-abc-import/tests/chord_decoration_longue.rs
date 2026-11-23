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

//! Décoration longue `!…!` à l'intérieur d'un accord (ABC 2.1 §4.17) :
//! « decorations which may be attached to individual notes within the chord ».
//! Réduit de `flattener.test__abcVolumeInChords.abc` (`[!pppp!c!ffff!D]`),
//! dont la lecture renvoyait zéro note. `abc2xml.py::chord_note` (lignes
//! 383-385) accepte `dec_note` — décoration puis hauteur — pour chaque note.

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
fn decoration_longue_dans_un_accord() {
    let source = "X:1\nM:4/4\nL:1/1\n[!pppp!c!ffff!D]\n";
    assert_eq!(pitches(source), vec![72, 62], "accord perdu :\n{source}");
}

#[test]
fn decoration_longue_ne_cree_pas_de_notes() {
    let source = "X:1\nM:4/4\nL:1/4\n[c!ffff!D]\n";
    assert_eq!(pitches(source), vec![72, 62], "faux sons :\n{source}");
}
