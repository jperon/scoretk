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

//! Diagnostic : imprime les n-olets et le nombre d'enfants.

use mscz2xml::model::SegmentItem;
use scoretk_musicxml::read_musicxml;

fn main() {
    let path = std::env::args().nth(1).expect("chemin");
    let score = read_musicxml(&std::fs::read(&path).expect("lecture")).expect("analyse");
    for (index, tuplet) in score.tuplets.iter().enumerate().take(16) {
        let mut chords = 0;
        let mut children = 0;
        for measure in &score.measures {
            for segment in &measure.segments {
                for (_, item) in segment.iter_elements() {
                    if let SegmentItem::ChordRest(chord_rest) = item {
                        if chord_rest.tuplet == Some(mscz2xml::model::TupletId::from_index(index)) {
                            chords += 1;
                            let _ = segment;
                        }
                    }
                }
            }
        }
        children += chords;
        children += score
            .tuplets
            .iter()
            .filter(|other| other.parent == Some(mscz2xml::model::TupletId::from_index(index)))
            .count();
        println!(
            "{index}: ratio {} tick {} len {} base {} normal {} children {} (chords {}) parent {:?}",
            tuplet.ratio, tuplet.tick, tuplet.len, tuplet.base_len, tuplet.normal_notes, children, chords, tuplet.parent
        );
    }
}
