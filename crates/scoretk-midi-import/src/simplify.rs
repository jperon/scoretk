// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_simplify.cpp` de MuseScore Studio, Copyright (C)
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

//! Simplification des durées : allonger les notes pour réduire les silences.
//!
//! Portage de `Simplify::simplifyDurations`, `minimizeNumberOfRests`,
//! `lengthenNote` et `shortenDrumNote` (`importmidi_simplify.cpp`). Une note
//! exportée à 95 % de sa durée (temps de tenue d'un piano) est quantifiée à
//! une valeur légèrement courte ; l'allongement au tick suivant qui demande
//! moins de figures et de silence est le comportement de MuseScore.

use scoretk_core::model::Fraction;

use super::chord::{max_note_off_time, MTrack, MidiNote};
use super::meter::{beat_length_raw, meter_to_duration_list_with_sig, DurationKind};
use super::midifile::UNITS_PER_WHOLE;
use super::quant::quantize_to_large;
use super::timesigmap::{fraction_from_ticks, TimeSigMap};
use super::tuplet::{find_tuplets_in_bar_for_duration, TupletData};

/// Nombre de figures d'une liste, un demi-point par figure pointée
/// (`MidiDuration::durationCount`).
fn duration_count(durations: &[(Fraction, scoretk_core::model::Duration)]) -> f64 {
    let mut count = durations.len() as f64;
    for (_, duration) in durations {
        if duration.dots > 0 {
            count += 0.5;
        }
    }
    count
}

/// Début de la mesure contenant un temps (`MidiBar::findBarStart`).
fn find_bar_start(sigmap: &TimeSigMap, time: Fraction) -> Fraction {
    let (bar_index, _, _) = sigmap.tick_values(time.ticks(UNITS_PER_WHOLE as u32));
    fraction_from_ticks(sigmap.bar2tick(bar_index, 0))
}

/// Vrai si la liste contient une figure à crochets multiples
/// (`hasComplexBeamedDurations`).
fn has_complex_beamed_durations(durations: &[(Fraction, scoretk_core::model::Duration)]) -> bool {
    durations
        .iter()
        .any(|(_, duration)| duration.kind.hooks() >= 2)
}

/// Allonge une note jusqu'à la fin qui minimise le nombre de figures et de
/// silences (`lengthenNote`).
#[allow(clippy::too_many_arguments)]
fn lengthen_note(
    note: &mut MidiNote,
    voice: usize,
    note_on_time: Fraction,
    duration_start: Fraction,
    end_time: Fraction,
    bar_start: Fraction,
    bar_fraction: Fraction,
    bar_numerator: i64,
    tuplets: &std::collections::BTreeMap<super::tuplet::TupletKey, TupletData>,
    track: &MTrack,
    is_drum_track: bool,
    use_dots: bool,
) {
    if end_time <= note.off_time {
        return;
    }
    let Some(quant) = note.off_time_quant else {
        return;
    };
    let tuplets_for_duration = find_tuplets_in_bar_for_duration(
        track,
        voice,
        bar_start,
        note.off_time,
        end_time - note.off_time,
    );

    let orig_note_durations = meter_to_duration_list_with_sig(
        duration_start - bar_start,
        note.off_time - bar_start,
        bar_fraction,
        bar_numerator,
        &tuplets_for_duration,
        DurationKind::Note,
        use_dots,
    );
    let orig_rest_durations = meter_to_duration_list_with_sig(
        note.off_time - bar_start,
        end_time - bar_start,
        bar_fraction,
        bar_numerator,
        &tuplets_for_duration,
        DurationKind::Rest,
        use_dots,
    );

    let mut min_note_duration_count = duration_count(&orig_note_durations);
    let mut min_rest_duration_count = duration_count(&orig_rest_durations);
    let mut best_off_time: Option<Fraction> = None;

    let step = if quant.is_positive() {
        quant
    } else {
        Fraction::new(1, 16)
    };
    let mut off_time = note.off_time + step;
    while off_time <= end_time {
        let note_durations = meter_to_duration_list_with_sig(
            duration_start - bar_start,
            off_time - bar_start,
            bar_fraction,
            bar_numerator,
            &tuplets_for_duration,
            DurationKind::Note,
            use_dots,
        );
        let note_duration_count = duration_count(&note_durations);
        let mut rest_duration_count = 0.0;
        if off_time < end_time {
            let rest_durations = meter_to_duration_list_with_sig(
                off_time - bar_start,
                end_time - bar_start,
                bar_fraction,
                bar_numerator,
                &tuplets_for_duration,
                DurationKind::Rest,
                use_dots,
            );
            rest_duration_count = duration_count(&rest_durations);
        }
        if note_duration_count + rest_duration_count
            < min_note_duration_count + min_rest_duration_count
            && (is_drum_track || note_duration_count <= 1.5)
        {
            min_note_duration_count = note_duration_count;
            min_rest_duration_count = rest_duration_count;
            best_off_time = Some(off_time);
        }
        off_time += step;
    }

    let Some(best_off_time) = best_off_time else {
        return;
    };

    let mut has_loss_of_accuracy = false;
    if !is_drum_track {
        let total = best_off_time - duration_start;
        let added_part = if total.is_zero() {
            0.0
        } else {
            ((best_off_time - note.off_time) / total).as_f64()
        };
        let staccato_tol = 0.3;
        if added_part >= staccato_tol {
            if note_on_time == duration_start && min_note_duration_count <= 1.5 {
                note.staccato = true;
            } else {
                has_loss_of_accuracy = true;
            }
        }
    }

    if !is_drum_track
        && has_loss_of_accuracy
        && (orig_note_durations.len() + orig_rest_durations.len()) as f64
            - (min_note_duration_count + min_rest_duration_count)
            <= 1.0
        && !has_complex_beamed_durations(&orig_note_durations)
        && !has_complex_beamed_durations(&orig_rest_durations)
    {
        return;
    }

    note.off_time = best_off_time;
    let _ = tuplets;
}

/// Raccourcit une note de percussion (`shortenDrumNote`).
fn shorten_drum_note(
    note: &mut MidiNote,
    on_time: Fraction,
    chord_is_in_tuplet: bool,
    tuplet: Option<&TupletData>,
    next: Option<(Fraction, bool, Option<TupletData>)>,
) {
    if chord_is_in_tuplet {
        if let Some(tuplet) = tuplet {
            let len = tuplet.len / Fraction::from_integer(i64::from(tuplet.tuplet_number));
            let new_off_time = (on_time + len).min(tuplet.on_time + tuplet.len);
            if note.off_time > new_off_time {
                note.off_time = new_off_time;
                note.is_in_tuplet = true;
                note.off_time_quant = Some(len);
            }
        }
        return;
    }
    if let Some((next_on_time, next_is_in_tuplet, next_tuplet)) = next {
        let len = Fraction::from_ticks(480 / 8, UNITS_PER_WHOLE as u32);
        let mut new_off_time = on_time + len;
        if next_is_in_tuplet {
            if let Some(tuplet) = next_tuplet {
                if tuplet.on_time < new_off_time {
                    new_off_time = tuplet.on_time;
                }
            }
        }
        if new_off_time < note.off_time {
            note.off_time = new_off_time;
            note.is_in_tuplet = false;
            note.off_time_quant = Some(len);
        }
        let _ = next_on_time;
    }
}

/// Réduit le nombre de silences en allongeant les notes
/// (`minimizeNumberOfRests`).
fn minimize_number_of_rests(
    track: &mut MTrack,
    sigmap: &TimeSigMap,
    is_drum_track: bool,
    use_dots: bool,
) {
    let keys = track.chords.keys();
    for key in keys {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        let voice = chord.voice;
        let chord_is_in_tuplet = chord.is_in_tuplet;
        let chord_tuplet = chord
            .tuplet
            .and_then(|key| track.tuplets.get(&key))
            .cloned();
        // La table est ordonnée par date : le premier accord de même voix après
        // `key` est le plus proche. Un `iter()` complet coûtait O(n²).
        let next = track
            .chords
            .next_in_voice(key, voice)
            .map(|(next_key, next_chord)| {
                (
                    next_key.0,
                    next_chord.is_in_tuplet,
                    next_chord
                        .tuplet
                        .and_then(|key| track.tuplets.get(&key))
                        .cloned(),
                )
            });
        let notes_count = chord.notes.len();
        for index in 0..notes_count {
            let Some(mut note) = track
                .chords
                .get(key)
                .and_then(|chord| chord.notes.get(index))
                .cloned()
            else {
                continue;
            };
            if is_drum_track {
                shorten_drum_note(
                    &mut note,
                    key.0,
                    chord_is_in_tuplet,
                    chord_tuplet.as_ref(),
                    next.clone(),
                );
            }
            let bar_start = find_bar_start(sigmap, note.off_time);
            let timesig = sigmap.timesig(bar_start.ticks(UNITS_PER_WHOLE as u32));
            let bar_fraction = timesig.measure_len();
            let mut duration_start = key.0.max(bar_start);
            if chord_is_in_tuplet {
                if let Some(tuplet) = &chord_tuplet {
                    if note.off_time >= tuplet.on_time + tuplet.len {
                        duration_start = tuplet.on_time + tuplet.len;
                    }
                }
            }
            let mut end_time = if bar_start == note.off_time {
                bar_start
            } else {
                bar_start + bar_fraction
            };
            if note.is_in_tuplet {
                if let Some(tuplet_key) = note.tuplet {
                    if let Some(tuplet) = track.tuplets.get(&tuplet_key) {
                        if note.off_time == tuplet.on_time + tuplet.len {
                            continue;
                        }
                        if is_drum_track {
                            end_time = tuplet.on_time + tuplet.len;
                        } else {
                            end_time = bar_start
                                + quantize_to_large(
                                    note.off_time - bar_start,
                                    tuplet.len
                                        / Fraction::from_integer(i64::from(tuplet.tuplet_number)),
                                );
                        }
                    }
                }
            }
            let beat_len = if is_drum_track || timesig.numerator == 12 {
                beat_length_raw(bar_fraction, timesig.numerator)
            } else {
                super::meter::beat_length(bar_fraction)
            };
            let beat_time = bar_start + quantize_to_large(note.off_time - bar_start, beat_len);
            if end_time > beat_time {
                end_time = beat_time;
            }
            if let Some((next_on_time, next_is_in_tuplet, next_tuplet)) = &next {
                if *next_on_time < end_time {
                    end_time = *next_on_time;
                }
                if *next_is_in_tuplet && !note.is_in_tuplet {
                    if let Some(tuplet) = next_tuplet {
                        if tuplet.on_time < end_time {
                            end_time = tuplet.on_time;
                        }
                    }
                }
            }
            lengthen_note(
                &mut note,
                voice,
                key.0,
                duration_start,
                end_time,
                bar_start,
                bar_fraction,
                timesig.numerator,
                &track.tuplets,
                track,
                is_drum_track,
                use_dots,
            );
            if let Some(chord) = track.chords.get_mut(key) {
                if let Some(target) = chord.notes.get_mut(index) {
                    *target = note;
                }
            }
        }
    }
    let _ = max_note_off_time(&[]);
}

/// Simplifie les durées des pistes non percussives
/// (`simplifyDurationsNotDrums`).
pub fn simplify_durations_not_drums(tracks: &mut [MTrack], sigmap: &TimeSigMap, use_dots: bool) {
    simplify_durations(tracks, sigmap, false, use_dots);
}

/// Simplifie les durées des pistes de percussion
/// (`simplifyDurationsForDrums`).
pub fn simplify_durations_for_drums(tracks: &mut [MTrack], sigmap: &TimeSigMap, use_dots: bool) {
    simplify_durations(tracks, sigmap, true, use_dots);
}

/// Simplifie les durées des pistes du type demandé (`simplifyDurations`).
fn simplify_durations(tracks: &mut [MTrack], sigmap: &TimeSigMap, drums: bool, use_dots: bool) {
    for track in tracks {
        if track.drum_track != drums || track.chords.is_empty() {
            continue;
        }
        minimize_number_of_rests(track, sigmap, drums, use_dots);
        super::tuplet::remove_empty_tuplets(track);
    }
}
