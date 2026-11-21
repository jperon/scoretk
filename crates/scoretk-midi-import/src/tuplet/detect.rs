// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_tuplet_detect.cpp` de MuseScore Studio, Copyright (C)
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

//! Détection des n-olets d'une mesure (`MidiTuplet::detectTuplets`).
//!
//! Chaque division métrique de la mesure est essayée avec les nombres de
//! n-olet autorisés ; `findTupletApproximation` choisit un accord par position
//! dont l'erreur de n-olet bat l'erreur de quantification régulière, puis
//! `isTupletAllowed` filtre par nombre de notes et erreur totale.

use scoretk_core::model::Fraction;

use crate::chord::{min_allowed_duration, ChordKey, MTrack};
use crate::meter::{beat_length_raw, divisions_of_bar_for_tuplets_raw, is_compound_num};
use crate::quant::{
    find_on_time_quant_error, find_quantized_tuplet_chord_on_time,
    find_quantized_tuplet_note_off_time, quant_for_len,
};

use super::{tuplet_limits, TupletData, TupletInfo};

/// Options de détection (`MidiOperations::TrackOp`).
#[derive(Clone, Copy)]
pub(super) struct DetectOptions {
    pub search_2plets: bool,
    pub search_3plets: bool,
    pub search_4plets: bool,
    pub search_5plets: bool,
    pub search_7plets: bool,
    pub search_9plets: bool,
    pub is_human: bool,
    pub simplify_durations: bool,
}

/// Plus courte durée de note quantifiée d'un intervalle
/// (`shortestQuantizedNoteInRange`, `importmidi_quant.cpp:135-155`).
fn shortest_quantized_len(
    track: &MTrack,
    keys: &[ChordKey],
    start_index: usize,
    end_index: usize,
) -> Fraction {
    let division = Fraction::from_integer(1);
    let mut min_duration = division;
    for chord_key in keys.iter().take(end_index).skip(start_index) {
        if let Some(chord) = track.chords.get(*chord_key) {
            for note in &chord.notes {
                let duration = note.off_time - chord_key.0;
                if duration < min_duration {
                    min_duration = duration;
                }
            }
        }
    }
    let min_allowed = min_allowed_duration();
    let mut shortest = division;
    while shortest > min_allowed {
        if shortest <= min_duration {
            break;
        }
        shortest *= Fraction::HALF;
    }
    shortest
}

/// Vrai si le n-olet est acceptable (`isTupletAllowed`,
/// `importmidi_tuplet_detect.cpp:34-85`).
fn is_tuplet_allowed(track: &MTrack, info: &TupletInfo, options: DetectOptions) -> bool {
    // Duolet et triolet : un premier accord seul doit remplir sa case.
    for number in [2, 3] {
        if info.tuplet_number == number && info.chords.len() == 1 && info.first_chord_index == 0 {
            let tuplet_note_len = info.len / Fraction::from_integer(i64::from(number));
            if let Some(chord_key) = info.chords.first() {
                if let Some(chord) = track.chords.get(*chord_key) {
                    for note in &chord.notes {
                        if (note.off_time - chord_key.0 - tuplet_note_len).abs()
                            > tuplet_note_len * Fraction::HALF
                        {
                            return false;
                        }
                    }
                }
            }
        }
    }

    let limits = tuplet_limits(info.tuplet_number);
    let min_allowed_note_count = if options.is_human {
        limits.min_note_count_human
    } else {
        limits.min_note_count
    };
    if (info.chords.len() as i32) < min_allowed_note_count {
        return false;
    }
    // Duolets et quadruplets : l'erreur de n-olet peut égaler l'erreur
    // régulière, la notation restant plus simple.
    if info.tuplet_number == 2 || info.tuplet_number == 4 {
        if info.tuplet_sum_error > info.regular_sum_error {
            return false;
        }
    } else if info.tuplet_sum_error >= info.regular_sum_error {
        return false;
    }
    let tuplet_note_len = info.len / Fraction::from_integer(i64::from(info.tuplet_number));
    for chord_key in &info.chords {
        let Some(chord) = track.chords.get(*chord_key) else {
            continue;
        };
        for note in &chord.notes {
            if note.off_time - chord_key.0 >= tuplet_note_len * Fraction::HALF {
                return true;
            }
        }
    }
    false
}

/// Nombres de n-olet à essayer pour une division (`findTupletNumbers`,
/// `importmidi_tuplet_detect.cpp:87-117`).
fn find_tuplet_numbers(
    div_len: Fraction,
    bar_numerator: i64,
    beat_len: Fraction,
    options: DetectOptions,
) -> Vec<i32> {
    let mut numbers = Vec::new();
    if is_compound_num(bar_numerator) && div_len == beat_len {
        if options.search_2plets {
            numbers.push(2);
        }
        if options.search_4plets {
            numbers.push(4);
        }
    } else {
        if options.search_3plets {
            numbers.push(3);
        }
        if options.search_5plets {
            numbers.push(5);
        }
        if options.search_7plets {
            numbers.push(7);
        }
        if options.search_9plets {
            numbers.push(9);
        }
    }
    numbers
}

/// Somme des silences entre les accords du n-olet (`findSumLengthOfRests`,
/// `importmidi_tuplet_detect.cpp:122-167`).
fn find_sum_length_of_rests(
    track: &MTrack,
    info: &TupletInfo,
    start_bar_tick: Fraction,
    options: DetectOptions,
) -> Fraction {
    let tuplet_end_time = info.on_time + info.len;
    let tuplet_note_len = info.len / Fraction::from_integer(i64::from(info.tuplet_number));
    let tuplet_data = TupletData {
        voice: 0,
        on_time: start_bar_tick,
        len: info.len,
        tuplet_number: info.tuplet_number,
        element_count: 0,
    };
    let ratio = tuplet_limits(info.tuplet_number).ratio;
    let mut beg = info.on_time;
    let mut sum_len = Fraction::ZERO;

    for chord_key in &info.chords {
        let Some(chord) = track.chords.get(*chord_key) else {
            continue;
        };
        let staccato = if options.simplify_durations {
            info.staccato_chords.get(chord_key).copied()
        } else {
            None
        };
        let chord_on_time = if chord_key.0 < start_bar_tick {
            start_bar_tick
        } else {
            find_quantized_tuplet_chord_on_time(chord_key.0, &tuplet_data, ratio)
        };
        if beg < chord_on_time {
            sum_len += chord_on_time - beg;
        }
        let mut max_off_time = Fraction::ZERO;
        for (index, note) in chord.notes.iter().enumerate() {
            let mut note_off_time = note.off_time;
            if staccato == Some(index) {
                note_off_time = chord_on_time + tuplet_note_len;
            }
            if note_off_time > max_off_time {
                max_off_time = note_off_time;
            }
        }
        beg = find_quantized_tuplet_note_off_time(chord_key.0, max_off_time, &tuplet_data, ratio).0;
        if beg >= tuplet_end_time {
            break;
        }
    }
    if beg < tuplet_end_time {
        sum_len += tuplet_end_time - beg;
    }
    sum_len
}

/// Choisit un accord par position du n-olet (`findTupletApproximation`,
/// `importmidi_tuplet_detect.cpp:169-258`).
#[allow(clippy::too_many_arguments)]
fn find_tuplet_approximation(
    track: &MTrack,
    keys: &[ChordKey],
    start_index: usize,
    end_index: usize,
    start_time: Fraction,
    tuplet_len: Fraction,
    tuplet_number: i32,
    basic_quant: Fraction,
) -> TupletInfo {
    let tuplet_note_len = tuplet_len / Fraction::from_integer(i64::from(tuplet_number));
    // (erreur de n-olet, différence erreur n-olet - erreur régulière).
    let mut candidates: Vec<(Fraction, Fraction, i32, ChordKey, Fraction)> = Vec::new();

    for pos_index in 0..tuplet_number {
        let tuplet_note_pos =
            start_time + tuplet_note_len * Fraction::from_integer(i64::from(pos_index));
        for chord_key in keys.iter().take(end_index).skip(start_index) {
            if chord_key.0 < tuplet_note_pos - tuplet_note_len * Fraction::HALF {
                continue;
            }
            if chord_key.0 > tuplet_note_pos + tuplet_note_len * Fraction::HALF {
                break;
            }
            let Some(chord) = track.chords.get(*chord_key) else {
                continue;
            };
            let tuplet_error = (chord_key.0 - tuplet_note_pos).abs();
            let regular_error = find_on_time_quant_error(chord_key.0, chord, basic_quant);
            let diff = tuplet_error - regular_error;
            candidates.push((tuplet_error, diff, pos_index, *chord_key, regular_error));
        }
    }
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut used_positions: Vec<i32> = Vec::new();
    let mut used_chords: Vec<ChordKey> = Vec::new();
    let mut diff_sum = Fraction::ZERO;
    let mut first_chord_index = 0;
    let mut info = TupletInfo {
        tuplet_number,
        on_time: start_time,
        len: tuplet_len,
        ..TupletInfo::default()
    };
    let mut first_on_time: Option<Fraction> = None;

    for (tuplet_error, diff, pos_index, chord_key, regular_error) in candidates {
        if diff_sum + diff > Fraction::ZERO {
            break;
        }
        if used_positions.contains(&pos_index) || used_chords.contains(&chord_key) {
            continue;
        }
        used_positions.push(pos_index);
        used_chords.push(chord_key);
        diff_sum += diff;
        info.chords.push(chord_key);
        info.tuplet_sum_error += tuplet_error;
        info.regular_sum_error += regular_error;
        if first_on_time.is_none_or(|first| chord_key.0 <= first) {
            first_on_time = Some(chord_key.0);
            first_chord_index = pos_index;
        }
    }
    info.chords.sort();
    info.first_chord_index = first_chord_index;
    info
}

/// Marque les notes trop courtes du n-olet (`detectStaccato`,
/// `importmidi_tuplet_detect.cpp:263-282`).
fn detect_staccato(track: &MTrack, info: &mut TupletInfo) {
    let limits = tuplet_limits(info.tuplet_number);
    if (info.chords.len() as i32) < limits.min_note_count_staccato {
        return;
    }
    let tuplet_note_len = info.len / Fraction::from_integer(i64::from(info.tuplet_number));
    for chord_key in &info.chords {
        let Some(chord) = track.chords.get(*chord_key) else {
            continue;
        };
        for (index, note) in chord.notes.iter().enumerate() {
            if note.off_time - chord_key.0 < tuplet_note_len * Fraction::HALF {
                info.staccato_chords.insert(*chord_key, index);
            }
        }
    }
}

/// Vrai si un accord étranger tombe entre deux positions du n-olet
/// (`haveChordsInTheMiddleBetweenTupletChords`,
/// `importmidi_tuplet_detect.cpp:289-312`).
fn have_chords_in_the_middle(
    track: &MTrack,
    keys: &[ChordKey],
    start_index: usize,
    end_index: usize,
    info: &TupletInfo,
) -> bool {
    let tuplet_note_len = info.len / Fraction::from_integer(i64::from(info.tuplet_number));
    for chord_key in keys.iter().take(end_index).skip(start_index) {
        if info.chords.contains(chord_key) {
            continue;
        }
        if track.chords.get(*chord_key).is_none() {
            continue;
        }
        for position in 0..info.tuplet_number {
            let pos = info.on_time
                + tuplet_note_len * Fraction::from_integer(i64::from(position))
                + tuplet_note_len * Fraction::HALF;
            if (pos - chord_key.0).abs() < tuplet_note_len * Fraction::HALF {
                return true;
            }
        }
    }
    false
}

/// Vrai si la case est au moins aussi longue que la quantification régulière
/// (`isTupletLenAllowed`, `importmidi_tuplet_detect.cpp:314-324`).
fn is_tuplet_len_allowed(
    track: &MTrack,
    keys: &[ChordKey],
    start_index: usize,
    end_index: usize,
    tuplet_len: Fraction,
    tuplet_number: i32,
    basic_quant: Fraction,
) -> bool {
    let tuplet_note_len = tuplet_len / Fraction::from_integer(i64::from(tuplet_number));
    let shortest = shortest_quantized_len(track, keys, start_index, end_index);
    let regular_quant = quant_for_len(shortest, basic_quant);
    tuplet_note_len >= regular_quant
}

/// Un seul accord de la mesure suivante peut appartenir au n-olet, et ce doit
/// être le dernier accord de la mesure courante (`isNextBarOwnershipOk`,
/// `importmidi_tuplet_detect.cpp:329-352`).
fn is_next_bar_ownership_ok(
    track: &MTrack,
    keys: &[ChordKey],
    start_index: usize,
    end_index: usize,
    bar_index: i32,
) -> bool {
    if end_index == 0 || end_index == start_index {
        return false;
    }
    let Some(last) = track.chords.get(keys[end_index - 1]) else {
        return false;
    };
    if last.bar_index == bar_index {
        return true;
    }
    let next_bar_counter = keys
        .iter()
        .take(end_index)
        .skip(start_index)
        .filter(|key| {
            track
                .chords
                .get(**key)
                .is_some_and(|chord| chord.bar_index > bar_index)
        })
        .count();
    next_bar_counter == 1
}

/// Détecte les n-olets d'une mesure (`detectTuplets`,
/// `importmidi_tuplet_detect.cpp:354-426`).
#[allow(clippy::too_many_arguments)]
pub(super) fn detect_tuplets(
    track: &MTrack,
    keys: &[ChordKey],
    bar_index: i32,
    bar_start_tick: Fraction,
    bar_len: Fraction,
    bar_numerator: i64,
    basic_quant: Fraction,
    options: DetectOptions,
) -> Vec<TupletInfo> {
    let mut div_lengths = divisions_of_bar_for_tuplets_raw(bar_len, bar_numerator);
    div_lengths.sort_by(|left, right| right.cmp(left));
    div_lengths.dedup();
    let beat_len = beat_length_raw(bar_len, bar_numerator);

    let mut tuplets = Vec::new();
    let mut id = 0;
    let tolerance = basic_quant * Fraction::HALF;

    for div_len in div_lengths {
        if !div_len.is_positive() {
            continue;
        }
        let tuplet_numbers = find_tuplet_numbers(div_len, bar_numerator, beat_len, options);
        let ratio = bar_len / div_len;
        let div_count = (ratio.numerator() / ratio.denominator()).max(0);
        for i in 0..div_count {
            let start_div_time = bar_start_tick + div_len * Fraction::from_integer(i);
            let end_div_time = bar_start_tick + div_len * Fraction::from_integer(i + 1);
            let start_with_tol = bar_start_tick.max(start_div_time - tolerance);

            // Accords de [start_with_tol, end_div_time).
            let start_index = keys.partition_point(|key| key.0 < start_with_tol);
            let end_index = keys.partition_point(|key| key.0 < end_div_time);
            if start_index >= end_index || end_index > keys.len() {
                continue;
            }
            let Some(first) = track.chords.get(keys[start_index]) else {
                continue;
            };
            if first.is_in_tuplet {
                continue;
            }
            if !is_next_bar_ownership_ok(track, keys, start_index, end_index, bar_index) {
                continue;
            }
            for &tuplet_number in &tuplet_numbers {
                if !is_tuplet_len_allowed(
                    track,
                    keys,
                    start_index,
                    end_index,
                    div_len,
                    tuplet_number,
                    basic_quant,
                ) {
                    continue;
                }
                let mut info = find_tuplet_approximation(
                    track,
                    keys,
                    start_index,
                    end_index,
                    start_div_time,
                    div_len,
                    tuplet_number,
                    basic_quant,
                );
                if options.simplify_durations
                    && !have_chords_in_the_middle(track, keys, start_index, end_index, &info)
                {
                    detect_staccato(track, &mut info);
                }
                info.sum_length_of_rests =
                    find_sum_length_of_rests(track, &info, bar_start_tick, options);
                if !is_tuplet_allowed(track, &info, options) {
                    continue;
                }
                info.id = id;
                id += 1;
                tuplets.push(info);
            }
        }
    }
    tuplets
}
