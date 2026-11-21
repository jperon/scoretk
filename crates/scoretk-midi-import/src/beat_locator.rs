// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_beat.cpp` de MuseScore Studio (Copyright (C)
// MuseScore Limited and others), modifié (réécrit en Rust) en 2026. Référence :
// `src/importexport/midi/internal/midiimport/importmidi_beat.cpp`.
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

//! Détection des temps d'une performance humaine et alignement des accords.

use std::collections::BTreeSet;

use scoretk_core::model::Fraction;

use crate::beatroot::{beat_track_with, AgentParameters, Event};
use crate::chord::MTrack;
use crate::meter::{beat_length_raw, division_info_raw, level_of_tick};
use crate::midifile::{EventKind, MidiFile, DIVISION, UNITS_PER_WHOLE};
use crate::timesigmap::{fraction_from_ticks, TimeSigMap, Timesig};

/// Données de temps humain (`MidiOperations::HumanBeatData`, champ beatSet).
#[derive(Clone, Debug)]
pub struct BeatData {
    pub beat_set: Vec<Fraction>,
    pub measure_count_2x_less: bool,
    pub time_sig: Timesig,
    /// `HumanBeatData::firstChordTick`.
    pub first_chord_tick: Fraction,
    /// `HumanBeatData::lastChordTick`.
    pub last_chord_tick: Fraction,
    /// `HumanBeatData::addedFirstBeats`.
    pub added_first_beats: i32,
    /// `HumanBeatData::addedLastBeats`.
    pub added_last_beats: i32,
}

fn ticks(fraction: Fraction) -> i64 {
    fraction.ticks(UNITS_PER_WHOLE as u32)
}

/// `MidiTempo::time2Tick` (`importmidi_tempo.cpp:26-28`).
fn time2tick(time: f64, ticks_per_sec: f64) -> Fraction {
    Fraction::from_ticks((ticks_per_sec * time) as i64, UNITS_PER_WHOLE as u32)
}

/// `MidiTempo::findBasicTempo(..., isHumanPerformance=true)` : les pistes sans
/// accord sont ignorées pour les fichiers humains (`importmidi_tempo.cpp:38-52`).
fn basic_tempo(_midi: &MidiFile, tracks: &[MTrack]) -> f64 {
    // `findBasicTempo(..., isHumanPerformance=true)` : ne pas lire le tempo
    // des pistes sans accords, souvent complètement erroné
    // (`importmidi_tempo.cpp:38-52`).
    for track in tracks {
        if track.chords.is_empty() {
            continue;
        }
        for (_, raw) in &track.events {
            if let EventKind::Meta { kind, data } = &raw.kind {
                if crate::midifile::meta::TEMPO == *kind && data.len() >= 3 {
                    let tempo =
                        (u32::from(data[0]) << 16) | (u32::from(data[1]) << 8) | u32::from(data[2]);
                    if tempo > 0 {
                        return 1_000_000.0 / f64::from(tempo);
                    }
                }
            }
        }
    }
    crate::tempo::DEFAULT_TEMPO
}

/// `findChordSalience1` (`importmidi_beat.cpp:42-67`).
fn salience1(on_time: Fraction, notes: &[crate::chord::MidiNote], ticks_per_sec: f64) -> f64 {
    let duration = notes
        .iter()
        .map(|note| note.off_time - on_time)
        .max()
        .unwrap_or(Fraction::ZERO);
    let duration_in_seconds = ticks(duration) as f64 / ticks_per_sec;
    let pitch = notes.iter().map(|note| note.pitch).min().unwrap_or(127);
    let velocity: i32 = notes.iter().map(|note| note.velocity).sum();
    let pitch = pitch.clamp(48, 72);
    let velocity = velocity.max(1);
    duration_in_seconds * (84.0 - f64::from(pitch)) * (velocity as f64).ln()
}

/// `findChordSalience2` (`importmidi_beat.cpp:69-79`).
fn salience2(notes: &[crate::chord::MidiNote]) -> f64 {
    notes.iter().map(|note| note.velocity).sum::<i32>().max(1) as f64
}

/// `prepareChordEvents` (`importmidi_beat.cpp:81-101`).
fn chord_events(track: &MTrack, ticks_per_sec: f64, use_salience1: bool) -> Vec<Event> {
    let mut events: Vec<Event> = track
        .chords
        .iter()
        .map(|(key, chord)| {
            let time = ticks(key) as f64 / ticks_per_sec;
            let salience = if use_salience1 {
                salience1(key, &chord.notes, ticks_per_sec)
            } else {
                salience2(&chord.notes)
            };
            Event::new(time, salience)
        })
        .collect();
    let min_salience = events
        .iter()
        .map(|event| event.salience)
        .fold(f64::MAX, f64::min);
    if min_salience < 0.0 {
        for event in &mut events {
            event.salience -= min_salience;
        }
    }
    events
}

/// `addFirstBeats` (`importmidi_beat.cpp:119-137`).
fn add_first_beats(set: &mut BTreeSet<Fraction>, first_tick: Fraction, beats_in_bar: i32) -> i32 {
    if set.is_empty() {
        return 0;
    }
    let mut added = 0;
    let first_beat = *set.iter().next().unwrap();
    if first_tick < first_beat && set.len() > 1 {
        let beat_len = *set.iter().nth(1).unwrap() - first_beat;
        let mut current = first_beat;
        loop {
            current -= beat_len;
            set.insert(current);
            added += 1;
            if !(current > first_tick || added % beats_in_bar != 0) {
                break;
            }
        }
    }
    added
}

/// `addLastBeats` (`importmidi_beat.cpp:149-168`).
fn add_last_beats(set: &mut BTreeSet<Fraction>, last_tick: Fraction, beats_in_bar: i32) -> i32 {
    if set.is_empty() {
        return 0;
    }
    let mut added = 0;
    let last_beat = *set.iter().next_back().unwrap();
    if last_tick > last_beat && set.len() > 1 {
        let mut it = set.iter().rev();
        let last = *it.next().unwrap();
        let prev = *it.next().unwrap();
        let beat_len = last - prev;
        let mut current = last_beat;
        loop {
            current += beat_len;
            set.insert(current);
            added += 1;
            if !(current < last_tick || added % beats_in_bar != 0) {
                break;
            }
        }
    }
    added
}

/// `findLastChordTick` (`importmidi_beat.cpp:103-112`).
fn last_chord_tick(track: &MTrack) -> Fraction {
    track
        .chords
        .iter()
        .flat_map(|(_, chord)| chord.notes.iter().map(|note| note.off_time))
        .max()
        .unwrap_or(Fraction::ZERO)
}

/// `findMatchRank` (`importmidi_beat.cpp:228-264`).
fn match_rank(
    set: &BTreeSet<Fraction>,
    events: &[Event],
    levels: &[i32],
    beats_in_bar: i32,
    ticks_per_sec: f64,
) -> f64 {
    let saliences: std::collections::BTreeMap<i64, f64> = events
        .iter()
        .map(|event| (ticks(time2tick(event.time, ticks_per_sec)), event.salience))
        .collect();
    let mut bar: Vec<i64> = Vec::new();
    let mut total = 0.0;
    let mut count = 0;
    for beat in set {
        bar.push(ticks(*beat));
        if bar.len() != beats_in_bar as usize {
            continue;
        }
        let mut relations = 0;
        let mut matches = 0;
        for i in 0..bar.len() - 1 {
            // `relationCount` compte toutes les paires, même sans saillance
            // (importmidi_beat.cpp:241-255).
            let s1 = saliences.get(&bar[i]);
            for j in (i + 1)..bar.len() {
                relations += 1;
                let Some(s1) = s1 else {
                    continue;
                };
                let Some(s2) = saliences.get(&bar[j]) else {
                    continue;
                };
                if (*s1 < *s2) == (levels[i] < levels[j]) {
                    matches += 1;
                }
            }
        }
        if relations > 0 {
            total += matches as f64 / relations as f64;
            count += 1;
        }
        bar.clear();
    }
    if count > 0 {
        total / count as f64
    } else {
        0.0
    }
}

/// `MidiBeat::findTimeSignatures` (`importmidi_beat.cpp:287-302`).
fn candidate_time_sigs(from_file: Timesig) -> Vec<Timesig> {
    let four = Timesig::new(4, 4);
    let three = Timesig::new(3, 4);
    if from_file == four || from_file == three {
        vec![four, three]
    } else {
        vec![from_file]
    }
}

fn metric_levels(bar_fraction: Fraction, numerator: i64, beat_len: Fraction) -> Vec<i32> {
    let info = division_info_raw(bar_fraction, numerator, &[]);
    let mut levels = Vec::new();
    let mut tick = Fraction::ZERO;
    while tick < bar_fraction {
        levels.push(level_of_tick(tick, &info));
        tick += beat_len;
    }
    levels
}

/// `MidiBeat::findBeatLocations` (`importmidi_beat.cpp:311-359`), avec les
/// options par défaut (les deux fonctionnalités de saillance, signatures
/// candidates 4/4 et 3/4).
pub fn find_beat_locations(
    track: &MTrack,
    sigmap: &TimeSigMap,
    ticks_per_sec: f64,
) -> Option<BeatData> {
    const MIN_BEAT_COUNT: usize = 8;
    let first_timesig = sigmap.timesig(0);
    let mut best_rank = f64::NEG_INFINITY;
    let mut best: Option<BeatData> = None;

    for use_salience1 in [true, false] {
        let events = chord_events(track, ticks_per_sec, use_salience1);
        let beat_times = beat_track_with(&events, AgentParameters::default(), &[]);
        if beat_times.len() <= MIN_BEAT_COUNT {
            continue;
        }
        for time_sig in candidate_time_sigs(first_timesig) {
            let beat_len = beat_length_raw(time_sig.measure_len(), time_sig.numerator);
            let beats_in_bar = (time_sig.measure_len() / beat_len).numerator();
            let levels = metric_levels(time_sig.measure_len(), time_sig.numerator, beat_len);
            let mut set: BTreeSet<Fraction> = beat_times
                .iter()
                .map(|time| time2tick(*time, ticks_per_sec))
                .collect();
            let added_first = add_first_beats(
                &mut set,
                track
                    .chords
                    .keys()
                    .first()
                    .map_or(Fraction::ZERO, |key| key.0),
                beats_in_bar as i32,
            );
            let added_last = add_last_beats(&mut set, last_chord_tick(track), beats_in_bar as i32);
            let rank = match_rank(&set, &events, &levels, beats_in_bar as i32, ticks_per_sec);
            if rank > best_rank {
                best_rank = rank;
                best = Some(BeatData {
                    beat_set: set.into_iter().collect(),
                    measure_count_2x_less: false,
                    time_sig,
                    first_chord_tick: track
                        .chords
                        .keys()
                        .first()
                        .map_or(Fraction::ZERO, |key| key.0),
                    last_chord_tick: last_chord_tick(track),
                    added_first_beats: added_first,
                    added_last_beats: added_last,
                });
            }
        }
    }
    best
}

/// `removeEvery2ndBeat` (`importmidi_beat.cpp:268-279`).
fn remove_every_2nd_beat(beats: &mut Vec<Fraction>) {
    let mut index = 0;
    while index < beats.len().saturating_sub(1) {
        beats.remove(index + 1);
        index += 1;
    }
    if index == beats.len() && beats.len() >= 2 {
        let beat_len = beats[beats.len() - 1] - beats[beats.len() - 2];
        beats.push(beats[beats.len() - 1] + beat_len);
    }
}

/// `MidiBeat::scaleOffTimes` (`importmidi_beat.cpp:400-432`).
fn scale_off_times(
    notes: &mut [crate::chord::MidiNote],
    beats: &[Fraction],
    end_index: usize,
    new_beat_start: Fraction,
    new_beat_len: Fraction,
) {
    for note in notes {
        let mut b_start = beats[end_index - 1];
        for (index, b_end) in beats.iter().enumerate().skip(end_index) {
            if note.off_time >= b_start && note.off_time < *b_end {
                let scale = new_beat_len / (*b_end - b_start);
                let mut new_off = (note.off_time - b_start) * scale;
                new_off =
                    crate::quant::quantize_value(new_off, crate::chord::min_allowed_duration());
                note.off_time = new_beat_start
                    + new_beat_len * Fraction::from_integer((index - end_index) as i64)
                    + new_off;
                break;
            }
            b_start = *b_end;
        }
    }
}

/// `MidiBeat::adjustChordsToBeats` (`importmidi_beat.cpp:412-467`).
pub fn adjust_chords_to_beats(tracks: &mut [MTrack], data: &BeatData) {
    let mut beats = data.beat_set.clone();
    if data.measure_count_2x_less {
        remove_every_2nd_beat(&mut beats);
    }
    if beats.len() < 2 {
        return;
    }
    let new_beat_len = Fraction::from_ticks(DIVISION, UNITS_PER_WHOLE as u32);
    for track in tracks.iter_mut() {
        if track.chords.is_empty() {
            continue;
        }
        let old: Vec<_> = track
            .chords
            .iter_with_keys()
            .map(|(key, chord)| (*key, chord.clone()))
            .collect();
        let mut new = crate::chord::ChordMap::new();
        let mut chord_index = 0;
        let mut beat_start = beats[0];
        let mut new_beat_start = Fraction::ZERO;
        for end_index in 1..beats.len() {
            let beat_end = beats[end_index];
            let scale = new_beat_len / (beat_end - beat_start);
            while chord_index < old.len() && old[chord_index].0 .0 < beat_end {
                let (key, mut chord) = old[chord_index].clone();
                let mut on_in_beat = (key.0 - beat_start) * scale;
                on_in_beat =
                    crate::quant::quantize_value(on_in_beat, crate::chord::min_allowed_duration());
                scale_off_times(
                    &mut chord.notes,
                    &beats,
                    end_index,
                    new_beat_start,
                    new_beat_len,
                );
                let on_time = new_beat_start + on_in_beat;
                for note in &mut chord.notes {
                    if note.off_time - on_time < crate::chord::min_allowed_duration() {
                        note.off_time = on_time + crate::chord::min_allowed_duration();
                    }
                }
                new.insert(on_time, chord);
                chord_index += 1;
            }
            if chord_index >= old.len() {
                break;
            }
            beat_start = beat_end;
            new_beat_start += new_beat_len;
        }
        track.chords = new;
    }
}

/// `MidiBeat::updateFirstLastBeats` (`importmidi_beat.cpp:472-497`) : les
/// temps ajoutés avec l'ancienne signature sont retirés puis recalculés avec
/// la nouvelle, afin que la grille commence et finisse sur une mesure entière.
fn update_first_last_beats(data: &mut BeatData) {
    for _ in 0..data.added_first_beats {
        if data.beat_set.is_empty() {
            break;
        }
        data.beat_set.remove(0);
    }
    for _ in 0..data.added_last_beats {
        if data.beat_set.is_empty() {
            break;
        }
        data.beat_set.pop();
    }
    let bar_fraction = data.time_sig.measure_len();
    let beat_len = beat_length_raw(bar_fraction, data.time_sig.numerator);
    let beats_in_bar = (bar_fraction / beat_len).numerator();
    let mut set: BTreeSet<Fraction> = data.beat_set.iter().copied().collect();
    data.added_first_beats = add_first_beats(&mut set, data.first_chord_tick, beats_in_bar as i32);
    data.added_last_beats = add_last_beats(&mut set, data.last_chord_tick, beats_in_bar as i32);
    data.beat_set = set.into_iter().collect();
}

/// `MidiBeat::setTimeSignature` (`importmidi_beat.cpp:499-511`).
pub fn set_time_sig(sigmap: &mut TimeSigMap, data: &mut BeatData) {
    if data.beat_set.is_empty() {
        return;
    }
    *sigmap = TimeSigMap::new();
    sigmap.add(0, data.time_sig);
    update_first_last_beats(data);
}

/// Temps en ticks par seconde pour `findBeatLocations`.
pub fn ticks_per_second(midi: &MidiFile, tracks: &[MTrack]) -> f64 {
    basic_tempo(midi, tracks) * DIVISION as f64
}

/// Référence de la première mesure (pour les tests).
pub fn first_bar_fraction(sigmap: &TimeSigMap) -> Fraction {
    fraction_from_ticks(sigmap.bar2tick(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::{MTrack, MidiChord, MidiNote};

    #[test]
    fn tempo_humain_ignore_la_piste_sans_accords() {
        // `human_tempo.mid` : piste 0 sans note avec des tempos erronés,
        // piste 1 avec les notes et sans tempo. `findBasicTempo(..., true)`
        // (`importmidi_tempo.cpp:38-52`) doit retomber sur 2 battements/s.
        let mef = MidiFile {
            tracks: vec![Default::default(), Default::default()],
            division: DIVISION,
            is_division_in_tps: false,
            format: 1,
        };
        let mut tempo_track = MTrack::default();
        tempo_track.events.push((
            0,
            crate::midifile::RawEvent {
                channel: 0,
                kind: EventKind::Meta {
                    kind: crate::midifile::meta::TEMPO,
                    data: vec![0x03, 0x93, 0x87], // 234375 µs
                },
            },
        ));
        let mut chord_track = MTrack::default();
        let mut chord = MidiChord::default();
        chord
            .notes
            .push(MidiNote::new(60, 90, Fraction::new(1, 4), Fraction::ZERO));
        chord_track.chords.insert(Fraction::ZERO, chord);
        let tps = ticks_per_second(&mef, &[tempo_track, chord_track]);
        assert!((tps - 960.0).abs() < 1e-9, "ticks/s {tps}");
    }
}
