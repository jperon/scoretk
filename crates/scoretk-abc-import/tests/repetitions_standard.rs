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

//! Marqueurs et sauts lus depuis les décorations standard de l'ABC 2.1
//! (§4.14), sans aucune directive `%%scoretk`.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{JumpType, MarkerType, RepeatMark};

#[test]
fn decorations_standard_deviennent_marqueurs_et_sauts() {
    let source = "\
X:1
T:Reprises
M:4/4
L:1/8
K:C
!segno!C8 |
D8!D.S.! |
!fine!E8 |
F8!D.C.! |]
";
    let score = read_abc(source).expect("lecture ABC");
    let kinds: Vec<_> = score
        .measures
        .iter()
        .flat_map(|measure| measure.repeat_marks.iter())
        .map(|mark| match mark {
            RepeatMark::Marker(marker) => format!("M:{:?}:{}", marker.kind, marker.label),
            RepeatMark::Jump(jump) => format!("J:{:?}:{}", jump.kind, jump.jump_to),
        })
        .collect();
    assert!(
        kinds.iter().any(|kind| kind == "M:Segno:segno"),
        "{kinds:?}"
    );
    assert!(kinds.iter().any(|kind| kind == "J:Ds:segno"), "{kinds:?}");
    assert!(kinds.iter().any(|kind| kind == "M:Fine:fine"), "{kinds:?}");
    assert!(kinds.iter().any(|kind| kind == "J:Dc:start"), "{kinds:?}");
    assert!(matches!(
        score.measures[0].repeat_marks.first(),
        Some(RepeatMark::Marker(marker)) if marker.kind == MarkerType::Segno
    ));
    assert!(score.measures.iter().any(|measure| {
        measure
            .repeat_marks
            .iter()
            .any(|mark| matches!(mark, RepeatMark::Jump(jump) if jump.kind == JumpType::Dc))
    }));
}
