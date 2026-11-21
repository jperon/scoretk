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
//! Portage de `importmidi_tuplet_filter.cpp` : élimination des n-olets
//! inutiles, groupe sans accords communs, puis recherche exhaustive avec les
//! intervalles de voix (`findNextTuplet`) et le critère d'erreur composite de
//! `TupletErrorResult`.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::Fraction;

use crate::chord::{ChordKey, MTrack};
use crate::quant::{find_max_quantized_off_time, find_on_time_quant_error};

use super::{tuplet_limits, TupletInfo};

/// Nombre maximal de n-olets avant filtrage (`MAX_TUPLETS`).
const MAX_TUPLETS: usize = 17;

/// Critère de comparaison des ensembles de n-olets (`TupletErrorResult`).
#[derive(Clone, Copy, Default)]
struct TupletErrorResult {
    tuplet_average_error: f64,
    relative_used_chord_places: f64,
    sum_length_of_rests: f64,
    voice_count: usize,
    tuplet_count: usize,
}

impl TupletErrorResult {
    fn is_initialized(self) -> bool {
        self.tuplet_count != 0
    }

    /// `operator<` du C++ (`importmidi_tuplet_filter.cpp:49-73`).
    fn less(self, other: Self) -> bool {
        fn div(left: f64, right: f64) -> f64 {
            if left == right {
                0.0
            } else {
                (left - right) / left.max(right)
            }
        }
        let mut value = div(self.tuplet_average_error, other.tuplet_average_error)
            - div(
                self.relative_used_chord_places,
                other.relative_used_chord_places,
            )
            + div(self.sum_length_of_rests, other.sum_length_of_rests);
        if value == 0.0 {
            value = div(self.voice_count as f64, other.voice_count as f64)
                + div(self.tuplet_count as f64, other.tuplet_count as f64);
        }
        value < 0.0
    }
}

/// `isMoreTupletVoicesAllowed`.
fn is_more_tuplet_voices_allowed(
    voices_in_use: usize,
    available_voices: usize,
    tuplet_voice_limit: usize,
) -> bool {
    !(voices_in_use >= available_voices || voices_in_use >= tuplet_voice_limit)
}

/// Vrai si deux n-olets partagent un accord (`haveCommonChords`).
fn have_common_chords(first: &TupletInfo, second: &TupletInfo) -> bool {
    first.chords.iter().any(|key| second.chords.contains(key))
}

/// Élimine les n-olets englobants qui contiennent les mêmes notes
/// (`removeUselessTuplets`, `:105-141`).
fn remove_useless_tuplets(tuplets: &mut Vec<TupletInfo>) {
    tuplets.sort_by(|left, right| {
        left.tuplet_number
            .cmp(&right.tuplet_number)
            .then(left.len.cmp(&right.len))
    });
    let mut beg = 0;
    while beg < tuplets.len() {
        let mut end = beg + 1;
        while end < tuplets.len() && tuplets[end].tuplet_number == tuplets[beg].tuplet_number {
            end += 1;
        }
        let mut i = beg;
        while i + 1 < end && i < tuplets.len() {
            let mut j = i + 1;
            while j < end && j < tuplets.len() {
                let t1 = tuplets[i].clone();
                let t2 = &tuplets[j];
                let contained = t1.on_time >= t2.on_time
                    && t1.on_time + t1.len <= t2.on_time + t2.len
                    && t2
                        .chords
                        .last()
                        .is_some_and(|key| key.0 < t1.on_time + t1.len)
                    && t2.chords.first().is_some_and(|key| key.0 >= t1.on_time);
                if contained {
                    tuplets.remove(j);
                    end -= 1;
                    continue;
                }
                j += 1;
            }
            i += 1;
        }
        beg = end;
    }
}

/// Intervalle couvert par un n-olet (`tupletInterval`, `importmidi_tuplet.cpp:252`).
fn tuplet_interval(
    track: &MTrack,
    tuplet: &TupletInfo,
    basic_quant: Fraction,
) -> (Fraction, Fraction) {
    let mut end = tuplet.on_time + tuplet.len;
    for key in &tuplet.chords {
        if let Some(chord) = track.chords.get(*key) {
            let off_time = find_max_quantized_off_time(chord, key.0, basic_quant);
            if off_time > end {
                end = off_time;
            }
        }
    }
    (tuplet.on_time, end)
}

/// Plus long groupe de n-olets sans accords communs
/// (`findLongestUncommonGroup`, `:143-195`).
fn find_longest_uncommon_group(
    track: &MTrack,
    tuplets: &[TupletInfo],
    basic_quant: Fraction,
) -> BTreeSet<usize> {
    let mut info: Vec<(Fraction, Fraction, usize)> = tuplets
        .iter()
        .enumerate()
        .map(|(index, tuplet)| {
            let (on_time, off_time) = tuplet_interval(track, tuplet, basic_quant);
            (on_time, off_time, index)
        })
        .collect();
    // Tri par fin croissante, puis par début décroissant ; `unique` sur la fin.
    info.sort_by(|left, right| left.1.cmp(&right.1).then(right.0.cmp(&left.0)));
    info.dedup_by(|left, right| left.1 == right.1);

    let mut indexes = BTreeSet::new();
    let mut last_selected = 0;
    for i in 0..info.len() {
        if i > 0 && info[i].0 < info[last_selected].1 {
            continue;
        }
        if have_common_chords(&tuplets[info[last_selected].2], &tuplets[info[i].2]) {
            continue;
        }
        last_selected = i;
        indexes.insert(info[i].2);
    }
    indexes
}

/// Vrai si deux n-olets ne peuvent pas coexister (`areInCommons`, `:207-222`).
fn are_in_commons(
    track: &MTrack,
    first: &TupletInfo,
    second: &TupletInfo,
    tuplet_voice_limit: usize,
) -> bool {
    for (first_index, first_key) in first.chords.iter().enumerate() {
        for (second_index, second_key) in second.chords.iter().enumerate() {
            if first_key != second_key {
                continue;
            }
            let notes = track
                .chords
                .get(*first_key)
                .map_or(0, |chord| chord.notes.len());
            if first.first_chord_index != 0
                || second.first_chord_index != 0
                || first_index != 0
                || second_index != 0
                || !is_more_tuplet_voices_allowed(1, notes, tuplet_voice_limit)
            {
                return true;
            }
        }
    }
    false
}

/// Table des incompatibilités (`findTupletCommons`, `:224-236`).
fn find_tuplet_commons(
    track: &MTrack,
    tuplets: &[TupletInfo],
    tuplet_voice_limit: usize,
) -> Vec<BTreeSet<usize>> {
    let mut commons = vec![BTreeSet::new(); tuplets.len()];
    if tuplets.len() < 2 {
        return commons;
    }
    for i in 0..tuplets.len() - 1 {
        for j in i + 1..tuplets.len() {
            if are_in_commons(track, &tuplets[i], &tuplets[j], tuplet_voice_limit) {
                commons[i].insert(j);
            }
        }
    }
    commons
}

/// Vrai si un indice est incompatible avec les n-olets retenus
/// (`isInCommonIndexes`, `:238-262`).
fn is_in_common_indexes(index: usize, selected: &[usize], commons: &[BTreeSet<usize>]) -> bool {
    for &tuplet_index in selected {
        if index > tuplet_index {
            if commons[tuplet_index].contains(&index) {
                return true;
            }
        } else if commons[index].contains(&tuplet_index) {
            return true;
        }
    }
    false
}

/// Critère d'erreur d'un ensemble de n-olets (`findTupletError`, `:264-313`).
fn find_tuplet_error(
    track: &MTrack,
    selected: &[usize],
    tuplets: &[TupletInfo],
    voice_count: usize,
    basic_quant: Fraction,
) -> TupletErrorResult {
    let mut sum_error = Fraction::ZERO;
    let mut sum_length_of_rests = Fraction::ZERO;
    let mut sum_chord_count = 0usize;
    let mut sum_chord_places = 0usize;
    let mut used_chords: BTreeSet<ChordKey> = BTreeSet::new();
    let mut used_indexes = vec![false; tuplets.len()];

    for &index in selected {
        let tuplet = &tuplets[index];
        sum_error += tuplet.tuplet_sum_error;
        sum_length_of_rests += tuplet.sum_length_of_rests;
        sum_chord_count += tuplet.chords.len();
        sum_chord_places += tuplet.tuplet_number.max(0) as usize;
        used_indexes[index] = true;
        used_chords.extend(tuplet.chords.iter().copied());
    }
    // Erreur de quantification des accords hors n-olets.
    for (index, tuplet) in tuplets.iter().enumerate() {
        if used_indexes[index] {
            continue;
        }
        for key in &tuplet.chords {
            if used_chords.contains(key) {
                continue;
            }
            if let Some(chord) = track.chords.get(*key) {
                sum_error += find_on_time_quant_error(key.0, chord, basic_quant);
            }
        }
    }

    let error = if sum_chord_count == 0 {
        0.0
    } else {
        fraction_value(sum_error) / sum_chord_count as f64
    };
    TupletErrorResult {
        tuplet_average_error: error,
        relative_used_chord_places: if sum_chord_places == 0 {
            0.0
        } else {
            sum_chord_count as f64 / sum_chord_places as f64
        },
        sum_length_of_rests: fraction_value(sum_length_of_rests),
        voice_count,
        tuplet_count: selected.len(),
    }
}

/// Valeur flottante d'une fraction.
fn fraction_value(value: Fraction) -> f64 {
    value.numerator() as f64 / value.denominator() as f64
}

/// Vrai si deux intervalles se recoupent (`haveIntersection`, strict).
fn have_intersection(first: (Fraction, Fraction), second: (Fraction, Fraction)) -> bool {
    first.1 > second.0 && first.0 < second.1
}

/// Première voix libre pour un n-olet (`findAvailableVoice`, `:346-364`).
fn find_available_voice(
    index: usize,
    tuplet_intervals: &[(Fraction, Fraction)],
    voice_intervals: &BTreeMap<usize, Vec<(Fraction, Fraction)>>,
) -> usize {
    let mut voice = 0;
    loop {
        if voice_intervals.get(&voice).is_some_and(|intervals| {
            intervals
                .iter()
                .any(|interval| have_intersection(tuplet_intervals[index], *interval))
        }) {
            voice += 1;
            continue;
        }
        return voice;
    }
}

/// Premiers accords utilisés par plusieurs n-olets (`prepareUsedFirstChords`).
fn prepare_used_first_chords(
    selected: &[usize],
    tuplets: &[TupletInfo],
) -> BTreeMap<ChordKey, usize> {
    let mut used = BTreeMap::new();
    for &index in selected {
        let tuplet = &tuplets[index];
        if tuplet.first_chord_index != 0 {
            continue;
        }
        if let Some(first) = tuplet.chords.first() {
            *used.entry(*first).or_insert(0) += 1;
        }
    }
    used
}

/// Indices non retenus inférieurs au dernier retenu (`findUnusedIndexes`).
fn find_unused_indexes(selected: &[usize]) -> Vec<usize> {
    let Some(&last) = selected.last() else {
        return Vec::new();
    };
    let mut unused = Vec::new();
    let mut position = 0;
    for index in 0..last {
        if position < selected.len() && index == selected[position] {
            position += 1;
            continue;
        }
        unused.push(index);
    }
    unused
}

/// Vrai si un n-olet peut rejoindre la sélection (`canUseIndex`, `:400-427`).
fn can_use_index(
    track: &MTrack,
    index: usize,
    tuplets: &[TupletInfo],
    tuplet_intervals: &[(Fraction, Fraction)],
    voice_intervals: &BTreeMap<usize, Vec<(Fraction, Fraction)>>,
    used_first_chords: &BTreeMap<ChordKey, usize>,
    tuplet_voice_limit: usize,
) -> bool {
    let tuplet = &tuplets[index];
    if tuplet.first_chord_index == 0 {
        if let Some(first) = tuplet.chords.first() {
            if let Some(&count) = used_first_chords.get(first) {
                let notes = track
                    .chords
                    .get(*first)
                    .map_or(0, |chord| chord.notes.len());
                if !is_more_tuplet_voices_allowed(count, notes, tuplet_voice_limit) {
                    return false;
                }
            }
        }
    }
    let voice = find_available_voice(index, tuplet_intervals, voice_intervals);
    let voice_count = voice_intervals.len().max(voice + 1);
    !(voice_count > 1
        && (tuplet.chords.len() as i32)
            < tuplet_limits(tuplet.tuplet_number).min_note_count_add_voice)
}

/// Met à jour le meilleur ensemble si l'erreur est moindre
/// (`tryUpdateBestIndexes`, `:470-486`).
fn try_update_best_indexes(
    best: &mut Vec<usize>,
    min_error: &mut TupletErrorResult,
    selected: &[usize],
    track: &MTrack,
    tuplets: &[TupletInfo],
    voice_intervals: &BTreeMap<usize, Vec<(Fraction, Fraction)>>,
    basic_quant: Fraction,
) {
    let error = find_tuplet_error(track, selected, tuplets, voice_intervals.len(), basic_quant);
    if !min_error.is_initialized() || error.less(*min_error) {
        *min_error = error;
        *best = selected.to_vec();
    }
}

/// Calcule les intervalles de voix (`prepareVoiceIntervals`, `:488-501`).
fn prepare_voice_intervals(
    selected: &[usize],
    tuplet_intervals: &[(Fraction, Fraction)],
) -> BTreeMap<usize, Vec<(Fraction, Fraction)>> {
    let mut voice_intervals: BTreeMap<usize, Vec<(Fraction, Fraction)>> = BTreeMap::new();
    for &index in selected {
        let voice = find_available_voice(index, tuplet_intervals, &voice_intervals);
        voice_intervals
            .entry(voice)
            .or_default()
            .push(tuplet_intervals[index]);
    }
    voice_intervals
}

/// Liste doublement chaînée des indices encore valides (`ValidTuplets`).
struct ValidTuplets {
    indexes: Vec<(i64, i64)>,
    first: i64,
}

impl ValidTuplets {
    fn new(size: usize) -> Self {
        let mut indexes = Vec::with_capacity(size);
        for index in 0..size as i64 {
            indexes.push((index - 1, index + 1));
        }
        Self { indexes, first: 0 }
    }

    fn first(&self) -> usize {
        self.first as usize
    }

    fn is_valid(&self, index: usize) -> bool {
        (index as i64) >= self.first && (index as i64) < self.indexes.len() as i64
    }

    fn next(&self, index: usize) -> usize {
        self.indexes[index].1 as usize
    }

    fn is_empty(&self) -> bool {
        self.first >= self.indexes.len() as i64
    }

    fn exclude(&mut self, index: usize) -> usize {
        if (index as i64) < self.first {
            return index;
        }
        let previous = self.indexes[index].0;
        let next = self.indexes[index].1;
        self.indexes[index] = (-1, self.indexes.len() as i64);
        if previous >= self.first {
            self.indexes[previous as usize].1 = next;
        }
        if (next as usize) < self.indexes.len() {
            self.indexes[next as usize].0 = previous;
        }
        if (index as i64) == self.first {
            self.first = next;
        }
        next as usize
    }

    fn save(&self) -> Vec<(i64, i64)> {
        self.indexes[self.first as usize..].to_vec()
    }

    fn restore(&mut self, saved: &[(i64, i64)]) {
        self.first = self.indexes.len() as i64 - saved.len() as i64;
        for (offset, value) in saved.iter().enumerate() {
            self.indexes[self.first as usize + offset] = *value;
        }
    }
}

/// Parcours récursif des ensembles compatibles (`findNextTuplet`, `:576-671`).
#[allow(clippy::too_many_arguments)]
fn find_next_tuplet(
    selected: &mut Vec<usize>,
    valid: &mut ValidTuplets,
    best: &mut Vec<usize>,
    min_error: &mut TupletErrorResult,
    track: &MTrack,
    commons: &[BTreeSet<usize>],
    tuplets: &[TupletInfo],
    tuplet_intervals: &[(Fraction, Fraction)],
    commons_size: usize,
    basic_quant: Fraction,
    tuplet_voice_limit: usize,
) {
    while !valid.is_empty() {
        let index = valid.first();
        let is_common_group_begins = selected.is_empty() && index == commons_size;
        if is_common_group_begins {
            for i in index..tuplets.len() {
                selected.push(i);
            }
        } else {
            selected.push(index);
        }

        let voice_intervals = prepare_voice_intervals(selected, tuplet_intervals);
        let used_first_chords = prepare_used_first_chords(selected, tuplets);

        if is_common_group_begins {
            let can_add_more = (0..commons_size).any(|i| {
                !is_in_common_indexes(i, selected, commons)
                    && can_use_index(
                        track,
                        i,
                        tuplets,
                        tuplet_intervals,
                        &voice_intervals,
                        &used_first_chords,
                        tuplet_voice_limit,
                    )
            });
            if !can_add_more {
                try_update_best_indexes(
                    best,
                    min_error,
                    selected,
                    track,
                    tuplets,
                    &voice_intervals,
                    basic_quant,
                );
            }
            return;
        }

        valid.exclude(index);
        let saved = valid.save();
        if !valid.is_empty() {
            for &i in &commons[index] {
                valid.exclude(i);
                if valid.is_empty() {
                    break;
                }
            }
        }
        let mut i = valid.first();
        while valid.is_valid(i) {
            if !can_use_index(
                track,
                i,
                tuplets,
                tuplet_intervals,
                &voice_intervals,
                &used_first_chords,
                tuplet_voice_limit,
            ) {
                i = valid.exclude(i);
                continue;
            }
            i = valid.next(i);
        }
        if valid.is_empty() {
            let unused = find_unused_indexes(selected);
            let can_add_more = unused.iter().any(|&i| {
                !is_in_common_indexes(i, selected, commons)
                    && can_use_index(
                        track,
                        i,
                        tuplets,
                        tuplet_intervals,
                        &voice_intervals,
                        &used_first_chords,
                        tuplet_voice_limit,
                    )
            });
            if !can_add_more {
                try_update_best_indexes(
                    best,
                    min_error,
                    selected,
                    track,
                    tuplets,
                    &voice_intervals,
                    basic_quant,
                );
            }
        } else {
            find_next_tuplet(
                selected,
                valid,
                best,
                min_error,
                track,
                commons,
                tuplets,
                tuplet_intervals,
                commons_size,
                basic_quant,
                tuplet_voice_limit,
            );
        }

        selected.pop();
        valid.restore(&saved);
    }
}

/// Déplace les n-olets incompatibles à la fin (`moveUncommonTupletsToEnd`).
fn move_uncommon_tuplets_to_end(tuplets: &mut [TupletInfo], uncommons: &mut BTreeSet<usize>) {
    let mut swap_with = tuplets.len().saturating_sub(1);
    let mut i = swap_with as i64;
    while i >= 0 {
        if uncommons.remove(&(i as usize)) {
            if i as usize != swap_with {
                tuplets.swap(i as usize, swap_with);
            }
            swap_with = swap_with.saturating_sub(1);
        }
        i -= 1;
    }
}

/// Recherche l'ensemble de n-olets de moindre erreur (`findBestTuplets`, `:691`).
fn find_best_tuplets(
    track: &MTrack,
    commons: &[BTreeSet<usize>],
    tuplets: &[TupletInfo],
    commons_size: usize,
    tuplet_intervals: &[(Fraction, Fraction)],
    basic_quant: Fraction,
    tuplet_voice_limit: usize,
) -> Vec<usize> {
    let mut best = Vec::new();
    let mut selected = Vec::new();
    let mut min_error = TupletErrorResult::default();
    let mut valid = ValidTuplets::new(tuplets.len());
    find_next_tuplet(
        &mut selected,
        &mut valid,
        &mut best,
        &mut min_error,
        track,
        commons,
        tuplets,
        tuplet_intervals,
        commons_size,
        basic_quant,
        tuplet_voice_limit,
    );
    best
}

/// Réduit le nombre de n-olets à `MAX_TUPLETS` (`removeExtraTuplets`, `:710`).
fn remove_extra_tuplets(track: &MTrack, tuplets: &mut Vec<TupletInfo>, basic_quant: Fraction) {
    if tuplets.len() <= MAX_TUPLETS {
        return;
    }
    let mut errors: Vec<(TupletErrorResult, usize)> = tuplets
        .iter()
        .enumerate()
        .map(|(index, tuplet)| {
            let chord_count = tuplet.chords.len().max(1);
            let places = tuplet.tuplet_number.max(1) as usize;
            (
                TupletErrorResult {
                    tuplet_average_error: fraction_value(tuplet.tuplet_sum_error)
                        / chord_count as f64,
                    relative_used_chord_places: chord_count as f64 / places as f64,
                    sum_length_of_rests: fraction_value(tuplet.sum_length_of_rests),
                    voice_count: 1,
                    tuplet_count: 1,
                },
                index,
            )
        })
        .collect();
    let _ = (track, basic_quant);
    errors.sort_by(|left, right| {
        if left.0.less(right.0) {
            std::cmp::Ordering::Less
        } else if right.0.less(left.0) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    let mut new_tuplets = Vec::new();
    for (_, index) in errors.into_iter().take(MAX_TUPLETS) {
        new_tuplets.push(tuplets[index].clone());
    }
    *tuplets = new_tuplets;
}

/// Filtre les n-olets détectés (`filterTuplets`, `:747-786`).
pub(super) fn filter_tuplets(
    track: &MTrack,
    tuplets: &mut Vec<TupletInfo>,
    basic_quant: Fraction,
    max_voice_count: usize,
) {
    if tuplets.is_empty() {
        return;
    }
    let tuplet_voice_limit = if max_voice_count <= 1 {
        1
    } else {
        max_voice_count - 1
    };
    remove_useless_tuplets(tuplets);
    remove_extra_tuplets(track, tuplets, basic_quant);

    let mut uncommons = find_longest_uncommon_group(track, tuplets, basic_quant);
    let mut commons_size = tuplets.len();
    if uncommons.len() > 1 {
        commons_size -= uncommons.len();
        move_uncommon_tuplets_to_end(tuplets, &mut uncommons);
    }
    let commons = find_tuplet_commons(track, tuplets, tuplet_voice_limit);
    let intervals: Vec<(Fraction, Fraction)> = tuplets
        .iter()
        .map(|tuplet| tuplet_interval(track, tuplet, basic_quant))
        .collect();
    let best = find_best_tuplets(
        track,
        &commons,
        tuplets,
        commons_size,
        &intervals,
        basic_quant,
        tuplet_voice_limit,
    );
    let mut kept = Vec::with_capacity(best.len());
    for index in best {
        kept.push(tuplets[index].clone());
    }
    *tuplets = kept;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::{MTrack, MidiChord, MidiNote};

    fn track_with_chords(onsets: &[Fraction]) -> (MTrack, Vec<ChordKey>) {
        let mut track = MTrack::default();
        let mut keys = Vec::new();
        for on_time in onsets {
            let mut chord = MidiChord::default();
            chord.notes.push(MidiNote::new(
                60,
                80,
                *on_time + Fraction::new(1, 4),
                *on_time,
            ));
            keys.push(track.chords.insert(*on_time, chord));
        }
        (track, keys)
    }

    fn tuplet(number: i32, on_time: Fraction, len: Fraction, chords: &[ChordKey]) -> TupletInfo {
        TupletInfo {
            tuplet_number: number,
            on_time,
            len,
            chords: chords.to_vec(),
            tuplet_sum_error: Fraction::ZERO,
            regular_sum_error: Fraction::new(1, 8),
            ..TupletInfo::default()
        }
    }

    #[test]
    fn nolet_englobant_retire() {
        // Même nombre de notes : le n-olet le plus grand qui contient les mêmes
        // accords est retiré (`removeUselessTuplets`). La sélection gloutonne
        // précédente gardait le premier venu, donc le plus grand.
        let onsets = [Fraction::ZERO, Fraction::new(1, 3), Fraction::new(2, 3)];
        let (track, keys) = track_with_chords(&onsets);
        let small = tuplet(3, Fraction::ZERO, Fraction::new(1, 1), &keys);
        let large = tuplet(3, Fraction::ZERO, Fraction::new(2, 1), &keys);
        let mut tuplets = vec![large, small];
        filter_tuplets(&track, &mut tuplets, Fraction::new(1, 16), 4);
        assert_eq!(tuplets.len(), 1, "un seul n-olet conservé");
        assert_eq!(
            tuplets[0].len,
            Fraction::new(1, 1),
            "le petit n-olet est conservé"
        );
    }
}
