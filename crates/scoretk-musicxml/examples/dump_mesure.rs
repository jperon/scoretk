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

//! Diagnostic : imprime les segments d'une mesure.

use mscz2xml::model::SegmentItem;
use scoretk_musicxml::read_musicxml;

fn main() {
    let path = std::env::args().nth(1).expect("chemin");
    let number: usize = std::env::args()
        .nth(2)
        .expect("numéro de mesure")
        .parse()
        .expect("nombre");
    let score = read_musicxml(&std::fs::read(&path).expect("lecture")).expect("analyse");
    let measure = &score.measures[number - 1];
    println!(
        "mesure {} tick {} len {} nominal {}",
        number, measure.start_tick, measure.len, measure.nominal_len
    );
    for segment in &measure.segments {
        for (track, item) in segment.iter_elements() {
            if let SegmentItem::ChordRest(chord_rest) = item {
                println!(
                    "{track} tick {} rest {} ticks {:?} dur {:?} tuplet {:?} gap {}",
                    segment.tick,
                    chord_rest.is_rest(),
                    chord_rest.ticks,
                    chord_rest.duration,
                    chord_rest.tuplet,
                    chord_rest.rest_ref().is_some_and(|rest| rest.gap)
                );
            }
        }
    }
}
