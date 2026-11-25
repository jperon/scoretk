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

//! Une barre ABC normale n'a pas de symbole, mais le modèle MuseScore porte
//! quand même un `BarLine` sur le segment final de chaque mesure.

use scoretk::model::{BarLineType, SegmentItem};
use scoretk::{read, Format};

#[test]
fn barre_implicite_en_fin_de_mesure() {
    let abc = b"X:1\nT:Test\nM:4/4\nL:1/8\nK:C\nC D E F|\n";
    let score = read(abc, Format::Abc).expect("lecture ABC");
    assert_eq!(score.measures.len(), 1);
    let found = score.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.iter_elements())
        .any(|(_, item)| matches!(item, SegmentItem::BarLine(bar) if bar.kind == BarLineType::Normal));
    assert!(found, "la barre implicite doit être un élément de segment");
}
