// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_voice.cpp` de MuseScore Studio, Copyright (C)
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

//! Séparation des voix d'une portée.
//!
//! Portage de `MidiVoice::separateVoices` (`importmidi_voice.cpp`) :
//! `doVoiceSeparation` choisit, pour chaque accord dont les notes n'ont pas
//! toutes la même durée (ou qui chevauche un autre accord de sa voix), le
//! meilleur point de coupe et la meilleure voix d'accueil, puis
//! `sortVoices`/`sortVoicesByPitch` renumérote les voix par hauteur moyenne
//! décroissante. Les n-olets suivent les accords déplacés.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::{Fraction, VOICES};

use super::chord::{self, ChordKey, MTrack, MidiNote};
use super::meter::{meter_to_duration_list_with_sig, DurationKind};
use super::midifile::UNITS_PER_WHOLE;
use super::operations::VoiceCount;
use super::simplify::{duration_count, find_bar_start};
use super::timesigmap::TimeSigMap;
use super::tuplet::{self, TupletKey};

/// Nombre de voix demandé (`toIntVoiceCount`).
fn to_int_voice_count(value: VoiceCount) -> usize {
    value.limit()
}

/// Groupe de notes déplacé lors d'une séparation (`MovedVoiceGroup`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum MovedVoiceGroup {
    Low,
    High,
}

/// Séparation possible d'un accord (`VoiceSplit`).
#[derive(Clone, Copy)]
struct VoiceSplit {
    group: MovedVoiceGroup,
    voice: usize,
}

/// Indication de mesure d'une mesure : longueur et numérateur brut.
fn measure_info(sigmap: &TimeSigMap, bar_start: Fraction) -> (Fraction, i64) {
    let timesig = sigmap.timesig(bar_start.ticks(UNITS_PER_WHOLE as u32));
    (timesig.measure_len(), timesig.numerator)
}

/// Index de mesure d'une fin de note (`findBarIndexForOffTime`,
/// `importmidi_voice.cpp:955-963`).
fn find_bar_index_for_off_time(sigmap: &TimeSigMap, off_time: Fraction) -> i32 {
    let (mut bar_index, beat, tick) = sigmap.tick_values(off_time.ticks(UNITS_PER_WHOLE as u32));
    if beat == 0 && tick == 0 {
        bar_index -= 1;
    }
    bar_index as i32
}

/// Vrai si toutes les notes de l'accord ont la même fin
/// (`allNotesHaveEqualLength`).
fn all_notes_have_equal_length(notes: &[MidiNote]) -> bool {
    let Some(first) = notes.first() else {
        return true;
    };
    notes.iter().all(|note| note.off_time == first.off_time)
}

/// Nombre de figures d'un groupe de notes (`findDurationCountInGroup`,
/// `importmidi_voice.cpp:138-200`).
fn find_duration_count_in_group(
    chord_on_time: Fraction,
    notes: &[MidiNote],
    voice: usize,
    group: &[usize],
    sigmap: &TimeSigMap,
    track: &MTrack,
    use_dots: bool,
) -> i32 {
    let mut count = 0;
    let mut on_time = chord_on_time;
    let mut on_bar_start = find_bar_start(sigmap, on_time);
    let (mut on_bar_fraction, mut on_bar_numerator) = measure_info(sigmap, on_bar_start);

    for &index in group {
        let Some(note) = notes.get(index) else {
            continue;
        };
        let off_time = note.off_time;
        if off_time == on_time {
            continue;
        }
        let off_bar_start = find_bar_start(sigmap, off_time);

        if off_bar_start != on_bar_start {
            let (off_bar_fraction, off_bar_numerator) = measure_info(sigmap, off_bar_start);
            let tuplets_for_duration = tuplet::find_tuplets_in_bar_for_duration(
                track,
                voice,
                on_bar_start,
                on_time,
                off_bar_start - on_time,
            );
            // Durées supplémentaires sur la frontière de mesure.
            let durations = meter_to_duration_list_with_sig(
                on_time - on_bar_start,
                off_bar_start - on_bar_start,
                off_bar_fraction,
                off_bar_numerator,
                &tuplets_for_duration,
                DurationKind::Note,
                use_dots,
            );
            count += duration_count(&durations) as i32;

            on_time = off_bar_start;
            on_bar_start = off_bar_start;
            on_bar_fraction = off_bar_fraction;
            on_bar_numerator = off_bar_numerator;
        }

        let tuplets_for_duration = tuplet::find_tuplets_in_bar_for_duration(
            track,
            voice,
            on_bar_start,
            on_time,
            off_time - on_time,
        );
        let durations = meter_to_duration_list_with_sig(
            on_time - on_bar_start,
            off_time - on_bar_start,
            on_bar_fraction,
            on_bar_numerator,
            &tuplets_for_duration,
            DurationKind::Note,
            use_dots,
        );
        count += duration_count(&durations) as i32;
        on_time = off_time;
    }
    count
}

/// Nombre total de figures si l'accord est coupé à `split_point`
/// (`findDurationCount`).
fn find_duration_count(
    notes: &[MidiNote],
    voice: usize,
    split_point: usize,
    chord_on_time: Fraction,
    sigmap: &TimeSigMap,
    track: &MTrack,
    use_dots: bool,
) -> i32 {
    let mut low_group: Vec<usize> = (0..split_point).collect();
    let mut high_group: Vec<usize> = (split_point..notes.len()).collect();
    low_group.sort_by_key(|index| notes[*index].off_time);
    high_group.sort_by_key(|index| notes[*index].off_time);

    find_duration_count_in_group(
        chord_on_time,
        notes,
        voice,
        &low_group,
        sigmap,
        track,
        use_dots,
    ) + find_duration_count_in_group(
        chord_on_time,
        notes,
        voice,
        &high_group,
        sigmap,
        track,
        use_dots,
    )
}

/// Meilleur point de coupe d'un accord (`findOptimalSplitPoint`,
/// `importmidi_voice.cpp:231-283`).
fn find_optimal_split_point(
    track: &MTrack,
    keys: &[ChordKey],
    index: usize,
    sigmap: &TimeSigMap,
    use_dots: bool,
) -> i32 {
    let Some(key) = keys.get(index).copied() else {
        return -1;
    };
    let Some(chord) = track.chords.get(key) else {
        return -1;
    };
    let notes = &chord.notes;

    if !all_notes_have_equal_length(notes) {
        let mut min_note_count = i32::MAX;
        let mut opt_split = -1;
        for split_point in 1..notes.len() {
            // Optimisation : ne pas couper entre deux notes de même durée.
            if notes[split_point - 1].off_time == notes[split_point].off_time {
                continue;
            }
            let note_count = find_duration_count(
                notes,
                chord.voice,
                split_point,
                key.0,
                sigmap,
                track,
                use_dots,
            );
            if note_count < min_note_count {
                min_note_count = note_count;
                opt_split = split_point as i32;
            }
        }
        opt_split
    } else {
        let off_time = notes.first().map_or(Fraction::ZERO, |note| note.off_time);
        for next_key in &keys[index + 1..] {
            if next_key.0 >= off_time {
                break;
            }
            let Some(next_chord) = track.chords.get(*next_key) else {
                continue;
            };
            if next_chord.voice != chord.voice {
                continue;
            }
            if next_key.0 < off_time {
                return 0;
            }
        }
        -1
    }
}

/// Fin de groupe la plus grande, n-olet compris (`maximizeOffTime`).
fn maximize_off_time(track: &MTrack, note: &MidiNote, off_time: Fraction) -> Fraction {
    let mut result = off_time;
    if note.off_time > off_time {
        result = note.off_time;
    }
    if note.is_in_tuplet {
        if let Some(tuplet) = note.tuplet.and_then(|key| track.tuplets.get(&key)) {
            if tuplet.on_time + tuplet.len > result {
                result = tuplet.on_time + tuplet.len;
            }
        }
    }
    result
}

/// N-olet inséré pour une voix donné à une date donnée (`findInsertedTuplet`).
fn find_inserted_tuplet(
    track: &MTrack,
    inserted: &BTreeMap<Fraction, Vec<TupletKey>>,
    on_time: Fraction,
    voice: usize,
) -> Option<TupletKey> {
    inserted.get(&on_time).and_then(|keys| {
        keys.iter().copied().find(|key| {
            track
                .tuplets
                .get(key)
                .is_some_and(|tuplet| tuplet.voice == voice)
        })
    })
}

/// Vrai si l'accord recoupe un n-olet qui ne vient pas d'être inséré pour
/// cette séparation (`hasIntersectionWithTuplets`,
/// `importmidi_voice.cpp:318-339`).
fn has_intersection_with_tuplets(
    track: &MTrack,
    voice: usize,
    on_time: Fraction,
    off_time: Fraction,
    inserted: &BTreeMap<Fraction, Vec<TupletKey>>,
    tuplet_on_time: Option<Fraction>,
) -> bool {
    for key in tuplet::find_tuplets_for_time_range(track, voice, on_time, off_time - on_time, true)
    {
        let Some(found) = track.tuplets.get(&key) else {
            continue;
        };
        let inserted_key = find_inserted_tuplet(track, inserted, found.on_time, voice);
        let belongs_to_inserted = inserted_key
            .and_then(|key| track.tuplets.get(&key))
            .is_some_and(|tuplet| Some(tuplet.on_time) == tuplet_on_time);
        if !belongs_to_inserted {
            return true;
        }
    }
    false
}

/// Ajoute les voix possibles pour un groupe de notes (`addGroupSplits`,
/// `importmidi_voice.cpp:341-381`).
#[allow(clippy::too_many_arguments)]
fn add_group_splits(
    track: &MTrack,
    splits: &mut Vec<VoiceSplit>,
    max_chord_length: Fraction,
    inserted: &BTreeMap<Fraction, Vec<TupletKey>>,
    tuplet_on_time: Option<Fraction>,
    on_time: Fraction,
    group_off_time: Fraction,
    orig_voice: usize,
    group_type: MovedVoiceGroup,
    max_occupied_voice: usize,
    limit: usize,
) {
    let mut split_added = false;

    for voice in 0..limit {
        if voice == orig_voice {
            continue;
        }
        if voice > max_occupied_voice && split_added {
            break;
        }
        if has_intersection_with_tuplets(
            track,
            voice,
            on_time,
            group_off_time,
            inserted,
            tuplet_on_time,
        ) {
            continue;
        }
        let found_chords = chord::find_chords_for_time_range(
            track,
            voice,
            on_time,
            group_off_time,
            max_chord_length,
        );
        if !found_chords.is_empty() {
            continue;
        }

        split_added = true;
        splits.push(VoiceSplit {
            group: group_type,
            voice,
        });
    }
}

/// Voix possibles pour un accord coupé à `split_point`
/// (`findPossibleVoiceSplits`, `importmidi_voice.cpp:398-451`).
#[allow(clippy::too_many_arguments)]
fn find_possible_voice_splits(
    track: &MTrack,
    keys: &[ChordKey],
    index: usize,
    split_point: usize,
    inserted: &BTreeMap<Fraction, Vec<TupletKey>>,
    max_occupied_voice: usize,
    limit: usize,
) -> Vec<VoiceSplit> {
    let Some(key) = keys.get(index).copied() else {
        return Vec::new();
    };
    let Some(chord) = track.chords.get(key) else {
        return Vec::new();
    };
    let mut splits = Vec::new();

    let mut on_time = key.0;
    let mut low_group_off_time = Fraction::ZERO;
    let mut high_group_off_time = Fraction::ZERO;

    let notes = &chord.notes;
    for note in notes.iter().take(split_point) {
        low_group_off_time = maximize_off_time(track, note, low_group_off_time);
    }
    for note in notes.iter().skip(split_point) {
        high_group_off_time = maximize_off_time(track, note, high_group_off_time);
    }

    let mut tuplet_on_time = None;
    if chord.is_in_tuplet {
        if let Some(tuplet) = chord.tuplet.and_then(|key| track.tuplets.get(&key)) {
            tuplet_on_time = Some(tuplet.on_time);
            if tuplet.on_time < on_time {
                on_time = tuplet.on_time;
            }
            if tuplet.on_time + tuplet.len > low_group_off_time {
                low_group_off_time = tuplet.on_time + tuplet.len;
            }
            if tuplet.on_time + tuplet.len > high_group_off_time {
                high_group_off_time = tuplet.on_time + tuplet.len;
            }
        }
    }

    let max_chord_length = chord::find_max_chord_length(track);

    if split_point > 0 {
        add_group_splits(
            track,
            &mut splits,
            max_chord_length,
            inserted,
            tuplet_on_time,
            on_time,
            low_group_off_time,
            chord.voice,
            MovedVoiceGroup::Low,
            max_occupied_voice,
            limit,
        );
    }
    if split_point < notes.len() {
        add_group_splits(
            track,
            &mut splits,
            max_chord_length,
            inserted,
            tuplet_on_time,
            on_time,
            high_group_off_time,
            chord.voice,
            MovedVoiceGroup::High,
            max_occupied_voice,
            limit,
        );
    }

    splits
}

/// Distance maximale de hauteur (`MAX_PITCH_DIST`).
const MAX_PITCH_DIST: i32 = 1000;
/// Octave, en demi-tons.
const OCTAVE: i32 = 12;

/// Distance de hauteur à l'accord précédent de la voix (`findPrevPitchDist`).
fn find_prev_pitch_dist(
    track: &MTrack,
    keys: &[ChordKey],
    index: usize,
    average_pitch: i32,
    voice: usize,
) -> i32 {
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        let Some(chord) = track.chords.get(keys[cursor]) else {
            continue;
        };
        if chord.voice == voice {
            return (chord::chord_average_pitch(chord) - average_pitch).abs();
        }
    }
    MAX_PITCH_DIST
}

/// Distance de hauteur à l'accord suivant de la voix (`findNextPitchDist`).
fn find_next_pitch_dist(
    track: &MTrack,
    keys: &[ChordKey],
    index: usize,
    average_pitch: i32,
    voice: usize,
) -> i32 {
    let mut cursor = index + 1;
    while cursor < keys.len() {
        let Some(chord) = track.chords.get(keys[cursor]) else {
            cursor += 1;
            continue;
        };
        if chord.voice == voice {
            return (chord::chord_average_pitch(chord) - average_pitch).abs();
        }
        cursor += 1;
    }
    MAX_PITCH_DIST
}

/// Distance de hauteur minimale à la voix, à une octave près
/// (`findMinPitchDist`).
fn find_min_pitch_dist(
    track: &MTrack,
    keys: &[ChordKey],
    index: usize,
    average_pitch: i32,
    voice: usize,
) -> i32 {
    let prev = find_prev_pitch_dist(track, keys, index, average_pitch, voice);
    let next = find_next_pitch_dist(track, keys, index, average_pitch, voice);

    if prev < next && prev <= OCTAVE {
        prev
    } else if next <= prev && next <= OCTAVE {
        next
    } else {
        MAX_PITCH_DIST
    }
}

/// Hauteur moyenne des notes sous le point de coupe
/// (`findAverageLowPitch`).
fn find_average_low_pitch(notes: &[MidiNote], split_point: usize) -> i32 {
    if split_point == 0 {
        return 0;
    }
    let sum: i32 = notes.iter().take(split_point).map(|note| note.pitch).sum();
    (f64::from(sum) / split_point as f64).round() as i32
}

/// Hauteur moyenne des notes au-dessus du point de coupe
/// (`findAverageHighPitch`).
fn find_average_high_pitch(notes: &[MidiNote], split_point: usize) -> i32 {
    let count = notes.len().saturating_sub(split_point);
    if count == 0 {
        return 0;
    }
    let sum: i32 = notes.iter().skip(split_point).map(|note| note.pitch).sum();
    (f64::from(sum) / count as f64).round() as i32
}

/// Meilleure séparation parmi les voix possibles (`findBestSplit`,
/// `importmidi_voice.cpp:530-571`).
fn find_best_split(
    track: &MTrack,
    keys: &[ChordKey],
    index: usize,
    possible_splits: &[VoiceSplit],
    split_point: usize,
) -> Option<VoiceSplit> {
    let key = keys.get(index).copied()?;
    let chord = track.chords.get(key)?;
    let notes = &chord.notes;
    let mut min_error = (i32::MAX, i32::MAX);
    let mut best_split = None;

    for split in possible_splits {
        let mut total_pitch_dist = 0;

        if split_point > 0 {
            let average_low_pitch = find_average_low_pitch(notes, split_point);
            let low_voice = if split.group == MovedVoiceGroup::Low {
                split.voice
            } else {
                chord.voice
            };
            total_pitch_dist +=
                find_min_pitch_dist(track, keys, index, average_low_pitch, low_voice);
        }

        if split_point < notes.len() {
            let average_high_pitch = find_average_high_pitch(notes, split_point);
            let high_voice = if split.group == MovedVoiceGroup::High {
                split.voice
            } else {
                chord.voice
            };
            total_pitch_dist +=
                find_min_pitch_dist(track, keys, index, average_high_pitch, high_voice);
        }

        let error = (total_pitch_dist, split.voice as i32);
        if error < min_error {
            min_error = error;
            best_split = Some(*split);
        }
    }

    best_split
}

/// Insère une copie d'un n-olet pour une nouvelle voix (`insertNewTuplet`,
/// `importmidi_voice.cpp:573-602`).
fn insert_new_tuplet(
    track: &mut MTrack,
    source_key: TupletKey,
    new_voice: usize,
    inserted: &mut BTreeMap<Fraction, Vec<TupletKey>>,
) -> Option<TupletKey> {
    // `importmidi_voice.cpp:578-580` copie `tuplet->second`, c'est-a-dire le
    // n-olet precis en cours de scission. Une recherche par date pouvait
    // choisir un autre n-olet simultane et copier sa longueur.
    let mut data = track.tuplets.get(&source_key)?.clone();
    let tuplet_on_time = data.on_time;
    data.voice = new_voice;
    let seq = track
        .tuplets
        .keys()
        .map(|key| key.1)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let key = (tuplet_on_time, seq);
    track.tuplets.insert(key, data);
    inserted.entry(tuplet_on_time).or_default().push(key);

    // Un accord hors n-olet à la date du n-olet rejoint la nouvelle copie.
    for chord_key in track.chords.keys_at(tuplet_on_time) {
        if let Some(chord) = track.chords.get_mut(chord_key) {
            if chord.voice == new_voice && !chord.is_in_tuplet {
                chord.is_in_tuplet = true;
                chord.tuplet = Some(key);
                break;
            }
        }
    }
    Some(key)
}

/// Vrai si le n-olet peut être scindé pour une nouvelle voix
/// (`canSplitTuplet`, `importmidi_voice.cpp:604-635`).
fn can_split_tuplet(
    track: &MTrack,
    tuplet_key: TupletKey,
    new_voice: usize,
    chord_on_time: Fraction,
    notes: &[MidiNote],
    inserted: &BTreeMap<Fraction, Vec<TupletKey>>,
    max_chord_length: Fraction,
) -> bool {
    let Some(tuplet) = track.tuplets.get(&tuplet_key) else {
        return true;
    };
    let tuplet_on_time = tuplet.on_time;
    let inserted_key = find_inserted_tuplet(track, inserted, tuplet_on_time, new_voice);

    let mut need_insert_tuplet = false;
    if inserted_key.is_none()
        && tuplet::has_non_trivial_chord(chord_on_time, notes, tuplet.on_time, tuplet.len)
    {
        need_insert_tuplet = true;
    }

    let need_delete_old_tuplet = tuplet::is_tuplet_useless(
        track,
        tuplet.voice,
        tuplet_on_time,
        tuplet.len,
        max_chord_length,
    );
    !(need_insert_tuplet && !need_delete_old_tuplet)
}

/// Scinde un n-olet entre deux voix (`splitTuplet`,
/// `importmidi_voice.cpp:637-702`).
#[allow(clippy::too_many_arguments)]
fn split_tuplet(
    track: &mut MTrack,
    tuplet_key: TupletKey,
    new_voice: usize,
    chord_on_time: Fraction,
    notes: &[MidiNote],
    is_in_tuplet: &mut bool,
    inserted: &mut BTreeMap<Fraction, Vec<TupletKey>>,
    max_chord_length: Fraction,
    allow_parallel_tuplets: bool,
    is_this_a_chord: bool,
) -> Option<TupletKey> {
    let old_key = tuplet_key;
    let Some(old_tuplet) = track.tuplets.get(&old_key).cloned() else {
        *is_in_tuplet = false;
        return None;
    };
    let tuplet_on_time = old_tuplet.on_time;
    let inserted_key = find_inserted_tuplet(track, inserted, tuplet_on_time, new_voice);

    let mut need_insert_tuplet = false;
    let mut current_key = old_key;
    match inserted_key {
        None => {
            if tuplet::has_non_trivial_chord(
                chord_on_time,
                notes,
                old_tuplet.on_time,
                old_tuplet.len,
            ) {
                need_insert_tuplet = true;
            } else {
                *is_in_tuplet = false;
            }
        }
        Some(key) => current_key = key,
    }

    let need_delete_old_tuplet = tuplet::is_tuplet_useless(
        track,
        old_tuplet.voice,
        old_tuplet.on_time,
        old_tuplet.len,
        max_chord_length,
    );
    let can_insert_tuplet = if allow_parallel_tuplets {
        need_insert_tuplet
    } else {
        need_insert_tuplet && need_delete_old_tuplet
    };
    if can_insert_tuplet {
        if let Some(key) = insert_new_tuplet(track, old_key, new_voice, inserted) {
            current_key = key;
            *is_in_tuplet = true;
        }
    }
    if need_delete_old_tuplet {
        let mut can_remove_tuplet = true;
        if is_this_a_chord {
            // Ne pas retirer le n-olet si ses notes y font encore référence :
            // il sera retiré lors de la scission des notes.
            for note in notes {
                if note.is_in_tuplet && note.tuplet == Some(old_key) {
                    can_remove_tuplet = false;
                    break;
                }
            }
        }
        if can_remove_tuplet {
            tuplet::remove_tuplet(track, old_key, max_chord_length);
        }
    }

    Some(current_key)
}

/// Met à jour les n-olets d'un accord déplacé (`updateChordTuplets`,
/// `importmidi_voice.cpp:704-752`).
fn update_chord_tuplets(
    track: &mut MTrack,
    chord: &mut super::chord::MidiChord,
    on_time: Fraction,
    inserted: &mut BTreeMap<Fraction, Vec<TupletKey>>,
    max_chord_length: Fraction,
    allow_parallel_tuplets: bool,
) -> bool {
    if !allow_parallel_tuplets {
        let mut can_do_split = true;
        if chord.is_in_tuplet {
            if let Some(key) = chord.tuplet {
                if !can_split_tuplet(
                    track,
                    key,
                    chord.voice,
                    on_time,
                    &chord.notes,
                    inserted,
                    max_chord_length,
                ) {
                    can_do_split = false;
                }
            }
        }
        if can_do_split {
            for note in &chord.notes {
                if !note.is_in_tuplet {
                    continue;
                }
                let Some(key) = note.tuplet else {
                    continue;
                };
                let single = [note.clone()];
                if !can_split_tuplet(
                    track,
                    key,
                    chord.voice,
                    on_time,
                    &single,
                    inserted,
                    max_chord_length,
                ) {
                    can_do_split = false;
                    break;
                }
            }
        }
        if !can_do_split {
            return false;
        }
    }

    if chord.is_in_tuplet {
        if let Some(key) = chord.tuplet {
            let notes = chord.notes.clone();
            let mut is_in_tuplet = chord.is_in_tuplet;
            let new_key = split_tuplet(
                track,
                key,
                chord.voice,
                on_time,
                &notes,
                &mut is_in_tuplet,
                inserted,
                max_chord_length,
                allow_parallel_tuplets,
                true,
            );
            chord.is_in_tuplet = is_in_tuplet;
            chord.tuplet = if is_in_tuplet { new_key } else { None };
        }
    }
    let chord_voice = chord.voice;
    for index in 0..chord.notes.len() {
        if !chord.notes[index].is_in_tuplet {
            continue;
        }
        let Some(key) = chord.notes[index].tuplet else {
            continue;
        };
        let single = [chord.notes[index].clone()];
        let mut is_in_tuplet = chord.notes[index].is_in_tuplet;
        let new_key = split_tuplet(
            track,
            key,
            chord_voice,
            on_time,
            &single,
            &mut is_in_tuplet,
            inserted,
            max_chord_length,
            allow_parallel_tuplets,
            false,
        );
        chord.notes[index].is_in_tuplet = is_in_tuplet;
        chord.notes[index].tuplet = if is_in_tuplet { new_key } else { None };
    }

    true
}

/// Voix maximale occupée dans une mesure (`findMaxOccupiedVoiceInBar`,
/// `importmidi_voice.cpp:754-786`).
fn find_max_occupied_voice_in_bar(track: &MTrack, bar_index: i32) -> usize {
    let mut max_voice = 0;
    for (_, chord) in track.chords.iter() {
        // `MidiVoice::findMaxOccupiedVoiceInBar` (importmidi_voice.cpp:754-784,
        // identique en 4.7.4) parcourt les mesures voisines pour bornes les
        // deux boucles, mais ne met `maxVoice` à jour **que** pour les accords
        // de la mesure courante (`chord.barIndex == barIndex`). Compter les
        // voix des mesures voisines autorisait à tort une voix supplémentaire
        // dans `addGroupSplits` (:354-386) et déplaçait 14 notes de
        // `benoit_ave_maria` en voix 2 contre MuseScore 4.7.4.
        if chord.bar_index == bar_index && chord.voice > max_voice {
            max_voice = chord.voice;
        }
    }
    max_voice
}

/// Déplace une partie des notes d'un accord vers une autre voix
/// (`splitChordToVoice`, `importmidi_voice.cpp:788-866`).
fn split_chord_to_voice(
    track: &mut MTrack,
    key: ChordKey,
    notes_to_move: &BTreeSet<usize>,
    new_voice: usize,
    inserted: &mut BTreeMap<Fraction, Vec<TupletKey>>,
    max_chord_length: Fraction,
) -> (bool, Option<ChordKey>) {
    if notes_to_move.is_empty() {
        return (false, None);
    }
    let Some(chord) = track.chords.get(key).cloned() else {
        return (false, None);
    };
    let on_time = key.0;

    if notes_to_move.len() == chord.notes.len() {
        // Pas de scission : l'accord entier change de voix.
        let old_voice = chord.voice;
        let mut moved = chord;
        moved.voice = new_voice;
        if update_chord_tuplets(
            track,
            &mut moved,
            on_time,
            inserted,
            max_chord_length,
            false,
        ) {
            track.chords.insert_at(key, moved);
            (true, None)
        } else {
            if let Some(target) = track.chords.get_mut(key) {
                target.voice = old_voice;
            }
            (false, None)
        }
    } else {
        let mut new_chord = chord.clone();
        new_chord.notes.clear();
        new_chord.voice = new_voice;
        let mut updated_old_notes = Vec::new();
        for (index, note) in chord.notes.iter().enumerate() {
            if notes_to_move.contains(&index) {
                new_chord.notes.push(note.clone());
            } else {
                updated_old_notes.push(note.clone());
            }
        }

        let remembered_notes = chord.notes.clone();
        if let Some(target) = track.chords.get_mut(key) {
            target.notes = updated_old_notes;
        }

        if update_chord_tuplets(
            track,
            &mut new_chord,
            on_time,
            inserted,
            max_chord_length,
            false,
        ) {
            if new_chord.notes.is_empty() {
                if let Some(target) = track.chords.get_mut(key) {
                    target.notes = remembered_notes;
                }
                return (false, None);
            }
            let new_key = track.chords.insert(on_time, new_chord);
            (true, Some(new_key))
        } else {
            if let Some(target) = track.chords.get_mut(key) {
                target.notes = remembered_notes;
            }
            (false, None)
        }
    }
}

/// Notes à déplacer selon le groupe et le point de coupe (`findNotesToMove`).
fn find_notes_to_move(
    notes_len: usize,
    split_point: usize,
    split_group: MovedVoiceGroup,
) -> BTreeSet<usize> {
    if split_point == 0 || split_point == notes_len {
        return (0..notes_len).collect();
    }
    match split_group {
        MovedVoiceGroup::Low => (0..split_point).collect(),
        MovedVoiceGroup::High => (split_point..notes_len).collect(),
    }
}

/// Sépare les accords en voix dans une piste (`doVoiceSeparation`,
/// `importmidi_voice.cpp:895-953`).
fn do_voice_separation(
    track: &mut MTrack,
    sigmap: &TimeSigMap,
    limit: usize,
    use_dots: bool,
) -> bool {
    chord::sort_notes_by_length(track);
    let mut inserted: BTreeMap<Fraction, Vec<TupletKey>> = BTreeMap::new();
    let keys = track.chords.keys();
    let max_chord_length = chord::find_max_chord_length(track);

    let mut changed = false;
    let mut max_occupied_voices: BTreeMap<i32, usize> = BTreeMap::new();
    let mut index = 0;
    while index < keys.len() {
        let key = keys[index];
        let Some(chord) = track.chords.get(key) else {
            index += 1;
            continue;
        };
        let bar_index = chord.bar_index;
        let split_point = find_optimal_split_point(track, &keys, index, sigmap, use_dots);
        if split_point == -1 {
            index += 1;
            continue;
        }

        let max_voice = *max_occupied_voices
            .entry(bar_index)
            .or_insert_with(|| find_max_occupied_voice_in_bar(track, bar_index));
        let possible_splits = find_possible_voice_splits(
            track,
            &keys,
            index,
            split_point as usize,
            &inserted,
            max_voice,
            limit,
        );
        if possible_splits.is_empty() {
            index += 1;
            continue;
        }

        let Some(best_split) =
            find_best_split(track, &keys, index, &possible_splits, split_point as usize)
        else {
            index += 1;
            continue;
        };
        changed = true;

        let notes_len = track.chords.get(key).map_or(0, |chord| chord.notes.len());
        let notes_to_move = find_notes_to_move(notes_len, split_point as usize, best_split.group);
        let (split_done, new_key) = split_chord_to_voice(
            track,
            key,
            &notes_to_move,
            best_split.voice,
            &mut inserted,
            max_chord_length,
        );
        if split_done && best_split.voice > max_voice {
            max_occupied_voices.insert(bar_index, best_split.voice);
        }
        if split_done && new_key.is_some() {
            // Comme le C++ après l'insertion : on reprend après tous les
            // accords de même date.
            index += 1;
            while index < keys.len() && keys[index].0 == key.0 {
                index += 1;
            }
        } else {
            index += 1;
        }
    }
    changed
}

/// Hauteur moyenne d'une liste d'accords (`averagePitchOfChords`).
fn average_pitch_of_chords(track: &MTrack, keys: &[ChordKey]) -> i32 {
    let mut sum_pitch = 0;
    let mut note_counter = 0;
    for key in keys {
        if let Some(chord) = track.chords.get(*key) {
            for note in &chord.notes {
                sum_pitch += note.pitch;
                note_counter += 1;
            }
        }
    }
    if note_counter == 0 {
        return -1;
    }
    (f64::from(sum_pitch) / f64::from(note_counter)).round() as i32
}

/// Renumérote les voix par hauteur moyenne décroissante
/// (`sortVoicesByPitch`, `importmidi_voice.cpp:985-1029`).
fn sort_voices_by_pitch(track: &mut MTrack, voice_chords: &BTreeMap<usize, Vec<ChordKey>>) {
    let mut pitch_voices: Vec<(i32, usize)> = voice_chords
        .iter()
        .map(|(voice, keys)| (average_pitch_of_chords(track, keys), *voice))
        .collect();
    pitch_voices.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));

    for (new_voice, (_, old_voice)) in pitch_voices.iter().enumerate() {
        if new_voice == *old_voice {
            continue;
        }
        let Some(keys) = voice_chords.get(old_voice) else {
            continue;
        };
        for key in keys {
            let Some(chord) = track.chords.get_mut(*key) else {
                continue;
            };
            chord.voice = new_voice;
            if chord.is_in_tuplet {
                if let Some(tuplet) = chord.tuplet.and_then(|key| track.tuplets.get_mut(&key)) {
                    tuplet.voice = new_voice;
                }
            }
            for note in &mut chord.notes {
                if note.is_in_tuplet {
                    if let Some(tuplet) = note.tuplet.and_then(|key| track.tuplets.get_mut(&key)) {
                        tuplet.voice = new_voice;
                    }
                }
            }
        }
    }
}

/// Trie les voix sur toute la partition, par blocs de mesures
/// (`sortVoices`, `importmidi_voice.cpp:1031-1063`).
fn sort_voices(track: &mut MTrack, sigmap: &TimeSigMap) {
    let keys = track.chords.keys();
    let mut voice_chords: BTreeMap<usize, Vec<ChordKey>> = BTreeMap::new();
    let mut max_bar_index: i32 = 0;

    for (index, key) in keys.iter().enumerate() {
        let Some(chord) = track.chords.get(*key) else {
            continue;
        };
        voice_chords.entry(chord.voice).or_default().push(*key);
        let bar_index = find_bar_index_for_off_time(sigmap, chord::max_note_off_time(&chord.notes));
        if bar_index > max_bar_index {
            max_bar_index = bar_index;
        }

        let next_bar = keys
            .get(index + 1)
            .and_then(|next| track.chords.get(*next))
            .map(|next| next.bar_index);
        if next_bar.is_none_or(|bar| bar > max_bar_index) {
            sort_voices_by_pitch(track, &voice_chords);
            voice_chords.clear();
        }
    }
}

/// Répartit les accords bruts en voix par chevauchement.
///
/// Équivalent de la répartition que `MidiTuplet::assignVoices` fait pendant la
/// détection des n-olets : la première voix libre (fin de note antérieure à
/// l'attaque) reçoit l'accord, sinon celle qui se libère le plus tôt. Sans
/// cette étape, toutes les couches rythmiques partageraient la voix 0 et la
/// détection des n-olets serait impossible.
pub fn assign_initial_voices(track: &mut MTrack, limit: usize) {
    assign_voices_impl(track, limit, None);
}

/// Variante après quantification : les fins de note sont alors sur la grille.
pub fn assign_initial_voices_quantized(track: &mut MTrack, limit: usize, basic_quant: Fraction) {
    assign_voices_impl(track, limit, Some(basic_quant));
}

fn assign_voices_impl(track: &mut MTrack, limit: usize, basic_quant: Option<Fraction>) {
    if track.drum_track || track.chords.is_empty() {
        return;
    }
    let limit = limit.clamp(1, VOICES);
    let keys = track.chords.keys();
    let mut voice_ends: Vec<Option<Fraction>> = vec![None; limit];
    for key in keys {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        let on_time = key.0;
        // `MidiTuplet::chordInterval` utilise `findMaxQuantizedOffTime` : une fin
        // de note à 44,542 tombe sur 44,5 et ne chevauche pas l'accord suivant
        // attaqué à 44,5. Comparer les fins brutes répartirait ces accords sur
        // deux voix et `minimizeNumberOfRests` allongerait ensuite la première
        // note d'un temps au lieu d'un croche (`chord_collect.mid`).
        let chord_end = match basic_quant {
            Some(bq) => chord
                .notes
                .iter()
                .map(|note| {
                    let quant = super::quant::quant_for_len(note.off_time - on_time, bq);
                    super::quant::quantize_value(note.off_time, quant)
                })
                .max()
                .unwrap_or(on_time),
            None => MTrack::max_off_time(chord),
        };
        let free = (0..limit).find(|voice| voice_ends[*voice].is_none_or(|end| end <= on_time));
        let voice = free.unwrap_or_else(|| {
            (0..limit)
                .min_by_key(|index| voice_ends[*index].unwrap_or(Fraction::ZERO))
                .unwrap_or(0)
        });
        voice_ends[voice] = Some(voice_ends[voice].map_or(chord_end, |end| end.max(chord_end)));
        if let Some(chord) = track.chords.get_mut(key) {
            chord.voice = voice;
        }
    }
}

/// Sépare les accords en voix (`MidiVoice::separateVoices`,
/// `importmidi_voice.cpp:1065-1121`).
///
/// Renvoie vrai si au moins une voix a changé, ce qui déclenche la seconde
/// simplification des durées dans le pipeline.
pub fn separate_voices(
    tracks: &mut [MTrack],
    sigmap: &TimeSigMap,
    max_voice_count: VoiceCount,
    use_dots: bool,
) -> bool {
    let limit = to_int_voice_count(max_voice_count).min(VOICES);
    let mut changed = false;

    for track in tracks {
        if track.drum_track || track.chords.is_empty() {
            continue;
        }
        if limit <= 1 {
            continue;
        }
        if do_voice_separation(track, sigmap, limit, use_dots) {
            changed = true;
        }
        sort_voices(track, sigmap);
    }

    changed
}

/// Attribue une voix aux accords hors n-olet (`MidiTuplet::setNonTupletVoices`,
/// `importmidi_tuplet_voice.cpp:171-206`).
///
/// MuseScore ne réserve pas l'intervalle des accords hors n-olet : seule la
/// voix doit être compatible avec les intervalles des n-olets, et la voix
/// retenue est celle dont la hauteur moyenne est la plus proche de celle des
/// n-olets qu'elle porte. Sans n-olet, toutes les voix ont la même distance
/// (`findPitchDist` renvoie `INT_MAX`), donc la voix 0 l'emporte : c'est l'état
/// d'entrée de `doVoiceSeparation`, qui fera la vraie répartition.
///
/// Approximation : la date de début de l'intervalle d'accord est sa date
/// d'attaque (`findOnTimeBetweenChords` n'est pas porté) ; la fin est la fin
/// brute de la plus longue note, les fins étant déjà quantifiées à ce stade.
pub fn assign_non_tuplet_voices(track: &mut MTrack, limit: usize) {
    if track.drum_track || track.chords.is_empty() {
        return;
    }
    let limit = limit.clamp(1, VOICES);

    let mut intervals: BTreeMap<usize, Vec<(Fraction, Fraction)>> = BTreeMap::new();
    for tuplet in track.tuplets.values() {
        intervals
            .entry(tuplet.voice)
            .or_default()
            .push((tuplet.on_time, tuplet.on_time + tuplet.len));
    }

    // `findPitchDist` : premier n-olet de la voix, hauteur moyenne de ses accords.
    let mut tuplet_pitch: BTreeMap<usize, i32> = BTreeMap::new();
    for (key, tuplet) in &track.tuplets {
        if tuplet_pitch.contains_key(&tuplet.voice) {
            continue;
        }
        let mut sum = 0;
        let mut count = 0;
        for (_, chord) in track.chords.iter() {
            let belongs = chord.tuplet == Some(*key)
                || chord.notes.iter().any(|note| note.tuplet == Some(*key));
            if belongs {
                for note in &chord.notes {
                    sum += note.pitch;
                    count += 1;
                }
            }
        }
        if count > 0 {
            tuplet_pitch.insert(
                tuplet.voice,
                (f64::from(sum) / f64::from(count)).round() as i32,
            );
        }
    }

    let keys = track.chords.keys();
    for key in keys {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        if chord.is_in_tuplet || chord.notes.iter().any(|note| note.is_in_tuplet) {
            continue;
        }
        let chord_pitch = chord::chord_average_pitch(chord);
        let interval = (key.0, MTrack::max_off_time(chord));
        let mut best_voice = 0;
        let mut min_dist: Option<i32> = None;
        for voice in 0..limit {
            let intersects = intervals.get(&voice).is_some_and(|list| {
                list.iter()
                    .any(|(on_time, off_time)| interval.1 > *on_time && interval.0 < *off_time)
            });
            if intersects {
                continue;
            }
            let dist = tuplet_pitch
                .get(&voice)
                .map_or(i32::MAX, |pitch| (chord_pitch - *pitch).abs());
            if min_dist.is_none_or(|current| dist < current) {
                min_dist = Some(dist);
                best_voice = voice;
            }
        }
        if let Some(chord) = track.chords.get_mut(key) {
            chord.voice = best_voice;
        }
    }
}

#[cfg(test)]
mod max_voice_tests {
    use super::*;
    use crate::chord::{MidiChord, MidiNote};

    /// `MidiVoice::findMaxOccupiedVoiceInBar` (`importmidi_voice.cpp:754-784`)
    /// regarde aussi la mesure précédente et la suivante ; le port ne
    /// regardait que la mesure courante, ce qui autorisait une voix de plus
    /// dans `addGroupSplits` (`:354-386`).
    /// `MidiVoice::insertNewTuplet` (`importmidi_voice.cpp:573-602`) copie
    /// `tuplet->second`, le n-olet precise en cours de scission. Le port
    /// cherchait d'abord un n-olet de la nouvelle voix a la meme date, puis
    /// n'importe quel n-olet de cette date : avec deux n-olets simultanes, il
    /// copiait le mauvais.
    #[test]
    fn copie_du_nolet_scinde_pas_d_un_autre_a_la_meme_date() {
        use crate::tuplet::TupletData;
        let mut track = MTrack::default();
        // Cle (date 0, rang 1) : le n-olet source, longueur 3/4, voix 0.
        let source = (Fraction::ZERO, 1);
        track.tuplets.insert(
            source,
            TupletData {
                voice: 0,
                on_time: Fraction::ZERO,
                len: Fraction::new(3, 4),
                tuplet_number: 3,
                element_count: 3,
            },
        );
        // Cle (date 0, rang 0) : un autre n-olet, deja dans la voix 2,
        // longueur 1/2. C'est celui que le port cherchait a tort.
        track.tuplets.insert(
            (Fraction::ZERO, 0),
            TupletData {
                voice: 2,
                on_time: Fraction::ZERO,
                len: Fraction::new(1, 2),
                tuplet_number: 3,
                element_count: 3,
            },
        );
        let notes = [MidiNote::new(60, 80, Fraction::new(1, 4), Fraction::ZERO)];
        let mut is_in_tuplet = true;
        let mut inserted = BTreeMap::new();
        super::split_tuplet(
            &mut track,
            source,
            2,
            Fraction::ZERO,
            &notes,
            &mut is_in_tuplet,
            &mut inserted,
            Fraction::new(1, 1),
            true,
            false,
        );

        assert!(
            track
                .tuplets
                .values()
                .any(|tuplet| tuplet.voice == 2 && tuplet.len == Fraction::new(3, 4)),
            "le nouveau n-olet doit copier la longueur 3/4 du n-olet scinde"
        );
    }

    #[test]
    fn voix_voisine_non_comptee_dans_max_occupe() {
        let mut track = MTrack::default();
        let mut first = MidiChord::default();
        first
            .notes
            .push(MidiNote::new(60, 80, Fraction::new(1, 2), Fraction::ZERO));
        first.bar_index = 0;
        first.voice = 1;
        track.chords.insert(Fraction::ZERO, first);
        let mut second = MidiChord::default();
        second.notes.push(MidiNote::new(
            64,
            80,
            Fraction::new(3, 2),
            Fraction::new(1, 1),
        ));
        second.bar_index = 1;
        second.voice = 0;
        track.chords.insert(Fraction::new(1, 1), second);
        // Les boucles du C++ parcourent les mesures voisines, mais ne
        // comptent que les accords de la mesure courante.
        assert_eq!(
            find_max_occupied_voice_in_bar(&track, 1),
            0,
            "la voix 1 de la mesure précédente ne limite pas les nouvelles voix"
        );
    }
}
