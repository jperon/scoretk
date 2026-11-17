// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rendering/score/measurelayout.cpp` et
// `engraving/rendering/score/systemlayout.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Systèmes explicites : portées masquées et barres de courtoisie.
//!
//! Le modèle ne fait pas de mise en page : les systèmes ne sont connus que par
//! les sauts écrits dans le `.mscx` (`<LayoutBreak>` de type `line` ou `page`,
//! cadres, sauts de section). Les ruptures automatiques calculées par
//! MuseScore restent hors de portée ; les écarts correspondants sont
//! documentés dans `docs/agents/ecarts-complets.md`.
//!
//! Deux comportements de la mise en page dépendent de ces systèmes :
//!
//! - `SystemLayout::hideEmptyStaves` (`systemlayout.cpp`) masque une portée
//!   vide sur tout un système (`<staff-details print-object="no"/>`) ;
//! - `MeasureLayout::createEndBarLines` (`measurelayout.cpp`) transforme la
//!   barre de fin en barre double (`light-light`) devant une armure de
//!   courtoisie, c'est-à-dire un changement de tonalité au début du système
//!   suivant (`keySigCourtesyBarlineMode` vaut `DOUBLE_BEFORE_COURTESY`).

use crate::model::{BarLineType, ChordRestKind, Key, Score, SegmentItem, SegmentType, Tick, Track};

/// Découpage en systèmes et portées masquées.
pub(crate) struct Systems {
    /// Portées masquées par système, indexées par la mesure qui ouvre le
    /// système (les autres entrées sont vides).
    hidden: Vec<Vec<usize>>,
    /// Vrai quand la mesure termine un système explicite.
    ends: Vec<bool>,
}

impl Systems {
    /// Calcule le découpage d'après les sauts explicites du modèle.
    pub(crate) fn new(score: &Score) -> Self {
        let count = score.measures.len();
        let mut starts = vec![false; count];
        let mut ends = vec![false; count];
        if let Some(first) = starts.first_mut() {
            *first = true;
        }
        for index in 1..count {
            let measure = &score.measures[index];
            let breaks = measure.line_break_before || measure.section_break_before;
            starts[index] = breaks;
            ends[index - 1] = breaks;
        }

        let mut hidden = vec![Vec::new(); count];
        if score.style.hide_empty_staves {
            // `SystemLayout::hideEmptyStaves` : le premier système n'est pas
            // concerné quand `dontHideStavesInFirstSystem` est actif.
            let mut begin = 0usize;
            for index in 1..=count {
                if index < count && !starts[index] {
                    continue;
                }
                let first_system = begin == 0;
                let hide = !(first_system && score.style.dont_hide_staves_in_first_system);
                if hide {
                    hidden[begin] = hidden_staves(score, begin, index);
                }
                begin = index;
            }
        }

        Self { hidden, ends }
    }

    /// Portées masquées du système qui ouvre la mesure `measure_index`.
    pub(crate) fn hidden_at(&self, measure_index: usize) -> &[usize] {
        self.hidden
            .get(measure_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Vrai si la mesure termine un système explicite.
    pub(crate) fn ends_system(&self, measure_index: usize) -> bool {
        self.ends.get(measure_index).copied().unwrap_or(false)
    }
}

/// Portées vides sur tout le système `[begin, end)` (`hideEmptyStaves`).
///
/// Une portée est masquée quand aucune voix ne porte d'accord sur le système et
/// qu'aucun accord croisé d'une portée voisine ne vient s'y poser
/// (`Measure::isEmpty`). `SystemLayout::computeShowSysStaff` regarde aussi les
/// spanners et les paroles ; ils ne sont pas modélisés ici, faute de mise en
/// page pour en décider.
fn hidden_staves(score: &Score, begin: usize, end: usize) -> Vec<usize> {
    let mut result = Vec::new();
    let mut first_visible = None;
    for staff in 0..score.staves.len() {
        if (begin..end).all(|index| measure_empty_for(score, index, staff)) {
            result.push(staff);
        } else if first_visible.is_none() {
            first_visible = Some(staff);
        }
    }
    // Un système entièrement vide montre sa première portée.
    if result.len() == score.staves.len() {
        if let Some(staff) = first_visible {
            result.retain(|hidden| *hidden != staff);
        }
    }
    result
}

/// Vrai si la mesure est vide pour une portée (`Measure::isEmpty`).
fn measure_empty_for(score: &Score, measure_index: usize, staff: usize) -> bool {
    let Some(measure) = score.measures.get(measure_index) else {
        return true;
    };
    if measure.multi_measure_rest.is_some() {
        return true;
    }
    for segment in &measure.segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        for (track, item) in segment.iter_elements() {
            let SegmentItem::ChordRest(chord_rest) = item else {
                continue;
            };
            let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                continue;
            };
            let raw = i64::from(track.staff().raw());
            let target = raw + i64::from(chord.staff_move);
            if target == i64::try_from(staff).unwrap_or(i64::MAX) {
                return false;
            }
        }
    }
    true
}

/// Vrai si une armure de courtoisie appelle une barre double à la fin de la
/// mesure `measure_index` (`createEndBarLines`).
///
/// La barre est double quand le système se termine là et que la tonalité change
/// au début du système suivant ; un saut de section masque la courtoisie par
/// défaut (`LayoutBreak::showCourtesy` vaut faux).
pub(crate) fn courtesy_key_bar_line(score: &Score, measure_index: usize) -> bool {
    let Some(measure) = score.measures.get(measure_index) else {
        return false;
    };
    if measure.end_bar_line != BarLineType::Normal {
        return false;
    }
    let Some(next) = score.measures.get(measure_index + 1) else {
        return false;
    };
    if next.section_break_before || !next.line_break_before {
        // Un saut de section ne crée pas de courtoisie ; sans saut de ligne
        // explicite, le système est découpé par la mise en page, inconnue ici.
        return false;
    }
    (0..score.staves.len()).any(|staff| {
        key_at(score, staff, measure.end_tick(), false)
            != key_at(score, staff, next.start_tick, true)
    })
}

/// Tonalité en vigueur pour une portée à un tick (`Staff::key`).
///
/// Les segments sont triés ; on retient la dernière armure antérieure au tick,
/// ou celle du tick quand `inclusive` est vrai.
fn key_at(score: &Score, staff: usize, tick: Tick, inclusive: bool) -> Key {
    let mut found = Key::C;
    let Some(staff_id) = score.staves.get(staff).map(|value| value.id) else {
        return found;
    };
    let track = Track::new(staff_id, 0);
    for measure in &score.measures {
        if measure.start_tick > tick {
            break;
        }
        for segment in &measure.segments {
            if segment.tick > tick || (!inclusive && segment.tick == tick) {
                break;
            }
            if !segment.kind.is_key_sig_type() {
                continue;
            }
            if let Some(SegmentItem::KeySig(key)) = segment.element(track) {
                found = key.transposed_key;
            }
        }
    }
    found
}
