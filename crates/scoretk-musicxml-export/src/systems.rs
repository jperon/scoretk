// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rendering/score/systemlayout.cpp` de MuseScore Studio,
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

//! Systèmes explicites : portées masquées (`hideEmptyStaves`).
//!
//! Le modèle ne fait pas de mise en page : les systèmes ne sont connus que par
//! les sauts écrits dans le `.mscx` (`Measure::line_break`, sauts de section et
//! cadres). Les ruptures automatiques calculées par MuseScore restent hors de
//! portée ; les écarts correspondants sont documentés dans
//! `docs/agents/ecarts-complets.md`.
//!
//! `SystemLayout::hideEmptyStaves` (`systemlayout.cpp`) masque une portée vide
//! sur tout un système ; l'export la signale par
//! `<staff-details print-object="no"/>` à la première mesure du système.

use scoretk_core::model::{ChordRestKind, Score, SegmentItem, SegmentType};

/// Portées masquées par système.
pub(crate) struct Systems {
    /// Portées masquées par système, indexées par la mesure qui ouvre le
    /// système (les autres entrées sont vides).
    hidden: Vec<Vec<usize>>,
}

impl Systems {
    /// Calcule le découpage d'après les sauts explicites du modèle.
    pub(crate) fn new(score: &Score) -> Self {
        let count = score.measures.len();
        let mut starts = vec![false; count];
        if let Some(first) = starts.first_mut() {
            *first = true;
        }
        for index in 1..count {
            // La mesure précédente finit un système (saut de ligne ou de page),
            // ou celle-ci ouvre une section.
            starts[index] =
                score.measures[index - 1].line_break || score.measures[index].section_break_before;
        }

        let mut hidden = vec![Vec::new(); count];
        if score.style.hide_empty_staves {
            // `SystemLayout::hideEmptyStaves` : le premier système n'est pas
            // concerné quand `dontHideStavesInFirstSystem` est actif.
            let mut begin = 0usize;
            for (index, start) in starts.iter().enumerate().skip(1) {
                if !*start {
                    continue;
                }
                hide_system(score, &mut hidden, begin, index);
                begin = index;
            }
            // Dernier système : jusqu'à la fin de la partition.
            hide_system(score, &mut hidden, begin, count);
        }

        Self { hidden }
    }

    /// Portées masquées du système qui ouvre la mesure `measure_index`.
    pub(crate) fn hidden_at(&self, measure_index: usize) -> &[usize] {
        self.hidden
            .get(measure_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// Calcule les portées masquées d'un système et les range à son début.
fn hide_system(score: &Score, hidden: &mut [Vec<usize>], begin: usize, end: usize) {
    let first_system = begin == 0;
    if first_system && score.style.dont_hide_staves_in_first_system {
        return;
    }
    hidden[begin] = hidden_staves(score, begin, end);
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
