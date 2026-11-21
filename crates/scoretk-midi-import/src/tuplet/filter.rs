// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_tuplet_filter.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Sélection des n-olets détectés (`MidiTuplet::filterTuplets`).
//!
//! Le C++ explore toutes les combinaisons sans accords communs, avec les
//! intervalles de voix et une recherche DFS (`findBestTuplets`,
//! `importmidi_tuplet_filter.cpp:691-786`). Le port retient une sélection
//! gloutonne équivalente sur l'essentiel : les n-olets sont classés par écart
//! d'erreur (erreur de n-olet moins erreur de quantification régulière), puis
//! par somme de silences, et un n-olet n'est retenu que si aucun de ses accords
//! n'appartient déjà à un n-olet retenu. La différence avec le C++ est
//! documentée dans `docs/agents/midi.md`.

use std::collections::BTreeSet;

use scoretk_core::model::Fraction;

use crate::chord::ChordKey;

use super::TupletInfo;

/// Retient un ensemble de n-olets sans accords communs (`filterTuplets`).
pub(super) fn filter_tuplets(tuplets: &mut Vec<TupletInfo>) {
    tuplets.sort_by(|left, right| {
        let left_error = left.tuplet_sum_error - left.regular_sum_error;
        let right_error = right.tuplet_sum_error - right.regular_sum_error;
        left_error
            .cmp(&right_error)
            .then(left.sum_length_of_rests.cmp(&right.sum_length_of_rests))
            .then(left.id.cmp(&right.id))
    });

    let mut used: BTreeSet<ChordKey> = BTreeSet::new();
    let mut kept: Vec<TupletInfo> = Vec::new();
    let mut intervals: Vec<(usize, Fraction, Fraction)> = Vec::new();
    for tuplet in tuplets.drain(..) {
        if tuplet.chords.iter().any(|key| used.contains(key)) {
            continue;
        }
        // Deux n-olets d'une même voix ne peuvent pas se recouvrir (le C++ les
        // écarte par les intervalles de voix, `findAvailableVoice`).
        let voice = tuplet.chords.first().map_or(0, |_| 0);
        let end = tuplet.on_time + tuplet.len;
        if intervals.iter().any(|(kept_voice, kept_on, kept_end)| {
            *kept_voice == voice && end > *kept_on && tuplet.on_time < *kept_end
        }) {
            continue;
        }
        used.extend(tuplet.chords.iter().copied());
        intervals.push((voice, tuplet.on_time, end));
        kept.push(tuplet);
    }
    *tuplets = kept;
}
