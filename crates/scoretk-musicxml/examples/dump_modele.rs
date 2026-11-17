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

//! Diagnostic : imprime les n-olets et les premiers accords du modèle.

use mscz2xml::model::SegmentItem;
use scoretk_musicxml::read_musicxml;

fn main() {
    let path = std::env::args().nth(1).expect("chemin");
    let bytes = std::fs::read(&path).expect("lecture");
    let score = read_musicxml(&bytes).expect("analyse");
    for (index, tuplet) in score.tuplets.iter().take(12).enumerate() {
        println!(
            "{index}: ratio {} tick {} len {} base {} parent {:?} actual {} normal {}",
            tuplet.ratio, tuplet.tick, tuplet.len, tuplet.base_len, tuplet.parent, tuplet.actual_notes, tuplet.normal_notes
        );
    }
    for (index, segment) in score.measures[0].segments.iter().enumerate().take(30) {
        println!("segment {index} {:?} tick {}", segment.kind, segment.tick);
        for (track, item) in segment.iter_elements() {
            if let SegmentItem::ChordRest(chord_rest) = item {
                println!(
                    "  {track} {:?} ticks {:?} dur {:?} tuplet {:?}",
                    chord_rest.is_rest(),
                    chord_rest.ticks,
                    chord_rest.duration,
                    chord_rest.tuplet
                );
            }
        }
    }
}
