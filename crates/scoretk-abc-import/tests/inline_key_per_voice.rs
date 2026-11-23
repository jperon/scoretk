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

//! Un champ `[K:…]` en ligne ne change que la voix courante.
//!
//! ABC 2.1 §7.3 (champs inline) : un champ de notation dans le corps s'applique
//! à la voix où il apparaît. `abc2xml.py` 269 traite chaque `inline` pendant
//! la construction de la partie (`doFields`, ligne 2029) et n'appelle
//! `doHeaderField` (2271) avec `K` que pour cette voix ; l'en-tête, lui, vaut
//! pour toutes les voix. L'air réduit vient de
//! `parsing.test__abcInlineKeyPerVoice.abc`.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Score, StaffId, Track};

const DEUX_VOIX: &str = "\
X:1
M:4/4
L:1/8
K:Edor
V:1
C2 D2 E2 F2| [K:F] G2 A2 B2 c2|]
V:2
C2 D2 E2 F2| [K:F] G2 A2 B2 c2|]
V:1
C2 D2 E2 F2| [K:Bb^f] G2 A2 B2 c2|]
V:2
C2 D2 E2 F2| [K:Bb^f] G2 A2 B2 c2|]
V:1
C2 D2 E2 F2| G2 A2 B2 c2|]
V:2
C2 D2 E2 F2| G2 A2 B2 c2|]
";

fn pitches(score: &Score, staff: usize) -> Vec<u8> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::new(StaffId::from_index(staff), 0)))
        .flat_map(|chord_rest| {
            chord_rest
                .chord_ref()
                .map(|chord| {
                    chord
                        .notes
                        .iter()
                        .map(|id| score.note(*id).pitch)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn armure_en_ligne_ne_change_pas_l_autre_voix() {
    let score = read_abc(DEUX_VOIX).expect("lecture");
    assert_eq!(
        pitches(&score, 0),
        vec![
            61, 62, 64, 66, 67, 69, 70, 72, // K:Edor, puis [K:F]
            60, 62, 64, 65, 67, 69, 70, 72, // [K:Bb^f]
            60, 62, 63, 65, 67, 69, 70, 72, // armure de la fin
        ],
        "V:1 : [K:F] puis [K:Bb^f] ne s'appliquent qu'à V:1"
    );
    assert_eq!(
        pitches(&score, 1),
        vec![
            61, 62, 64, 66, 67, 69, 70, 72, // K:Edor, puis [K:F]
            60, 62, 64, 65, 67, 69, 70, 72, // [K:Bb^f]
            60, 62, 63, 65, 67, 69, 70, 72, // armure de la fin
        ],
        "V:2 porte les mêmes champs [K:] mais ses propres changements, pas ceux de V:1"
    );
}
