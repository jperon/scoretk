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

//! ABC 2.1 §4.8 : `|]` et `||` portent des barres finales/doubles. Comme
//! MuseScore, le lecteur les met à la fois sur la mesure et sur son segment
//! `EndBarLine`, afin qu'un aller-retour ne perde pas le second.

use scoretk::model::{BarLineType, SegmentItem, SegmentType};
use scoretk::{read, Format};

fn barre_du_segment(src: &[u8]) -> BarLineType {
    let score = read(src, Format::Abc).expect("lecture ABC");
    let measure = &score.measures[0];
    assert_eq!(measure.end_bar_line, measure_end_bar_attendu(src));
    measure
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::EndBarLine)
        .and_then(|segment| {
            segment.iter_elements().find_map(|(_, item)| match item {
                SegmentItem::BarLine(bar) => Some(bar.kind),
                _ => None,
            })
        })
        .expect("un élément de barre de fin")
}

fn measure_end_bar_attendu(src: &[u8]) -> BarLineType {
    if src.windows(2).any(|w| w == b"|]") {
        BarLineType::End
    } else {
        BarLineType::Double
    }
}

#[test]
fn barre_finale_est_portee_par_le_segment() {
    assert_eq!(barre_du_segment(b"X:1\nK:C\nC4|]\n"), BarLineType::End);
}

#[test]
fn barre_double_est_portee_par_le_segment() {
    assert_eq!(barre_du_segment(b"X:1\nK:C\nC4||\n"), BarLineType::Double);
}
