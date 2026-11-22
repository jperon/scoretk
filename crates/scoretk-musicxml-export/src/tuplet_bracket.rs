// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Tuplet::calcHasBracket` (`src/engraving/dom/tuplet.cpp:204-268`)
// de MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Décision de crochet des n-olets quand `<bracketType>` est `AUTO_BRACKET`.
//!
//! `Tuplet::calcHasBracket` (`tuplet.cpp:204-268`) dépend des ligatures
//! construites par la mise en page : un n-olet entièrement ligaturé n'a pas de
//! crochet. Le modèle pivot ne porte pas les groupes de ligatures, mais porte
//! les durées notées : une ligature n'est possible que pour une croche ou plus
//! court (`:237-245`, `beamCount < 1` → crochet). On reproduit ce cas, les
//! autres conduisant au crochet comme dans le C++ (`:209-221`, `:231-236`).

use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem, SegmentType, TupletId};

/// Vrai si MuseScore dessinerait un crochet autour du n-olet.
pub(crate) fn has_bracket(score: &Score, id: TupletId) -> bool {
    let tuplet = &score.tuplets[id.index()];
    // Un ratio non réduit gardé tel quel (`4:4`) est traité comme un n-olet
    // spécial par le C++ : ses groupes de ligatures ne le font pas disparaître.
    let unreduced = tuplet.actual_notes > 1 && tuplet.actual_notes == tuplet.normal_notes;
    let mut elements = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.kind != SegmentType::ChordRest {
                continue;
            }
            for (track, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(chord_rest) = item {
                    if chord_rest.tuplet == Some(id) {
                        elements.push((chord_rest, score.staff(track.staff()).visible));
                    }
                }
            }
        }
    }
    // `cr1 == cr2` (n-olet dégénéré) : pas de crochet (`:209-211`).
    if elements.len() < 2 {
        return false;
    }
    // Une portée masquée n'est pas disposée : `hasBracket` reste à sa valeur
    // par défaut (faux), même pour un triolet avec silence. C'est ce qui
    // sépare les 31 crochets de Douce nuit (portée visible Femmes) des 31
    // triolets identiques sur portées masquées.
    if elements.iter().all(|(_, visible)| !visible) {
        return false;
    }
    // Le C++ 5.0 compte le silence comme ligaturable (`:232-234`) : sur une
    // portée visible, MuseScore 4.7.4 écrit le crochet.
    // les triolets de Douce nuit (2 croches 16es + 1 silence) n'ont pas de
    // crochet. Le C++ 5.0 (`tuplet.cpp:232-234`) traitait encore le silence
    // comme un élément ligaturable, mais la référence 4.7.4 prévaut.
    // Sans ligature possible (durée plus longue qu'une croche), le C++ garde
    // aussi le crochet (`:221`, `:231-245`).
    let limit = if unreduced {
        Fraction::new(1, 16)
    } else {
        Fraction::new(1, 8)
    };
    !elements.iter().all(|(element, _)| {
        matches!(element.kind, ChordRestKind::Chord(_)) && element.duration.ticks() <= limit
    })
}
