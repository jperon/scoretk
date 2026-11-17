// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_tempo.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. Le tempo
// d'une performance humaine, déduit des battements (`MidiBeat`), n'est pas
// porté : les performances détectées comme humaines n'ont pas de battements
// ajustés.
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

//! Tempo de l'import MIDI : lecture des méta-évènements et pose des indications.
//!
//! Portage de `MidiTempo::setTempo`, `findBasicTempo` et
//! `applyAllTempoEvents` (`importmidi_tempo.cpp`) : les tempos sont arrondis au
//! battement par minute, les répétitions sont ignorées, et le tick 0 reçoit
//! toujours son indication.

use mscz2xml::model::{Annotation, Fraction, Score, Segment, SegmentType, Tempo, Track};

use super::midifile::{meta, to_muse_score_ticks, EventKind, MidiFile, UNITS_PER_WHOLE};
use super::operations::ImportOptions;
use super::timesigmap::TimeSigMap;

/// Tempo par défaut, en battements par seconde (`Constants::DEFAULT_TEMPO`).
pub const DEFAULT_TEMPO: f64 = 2.0;

/// Tempo de base d'un fichier (`findBasicTempo`).
pub fn find_basic_tempo(midi: &MidiFile) -> f64 {
    for track in &midi.tracks {
        for (_, raw) in &track.events {
            if let EventKind::Meta { kind, data } = &raw.kind {
                if *kind == meta::TEMPO && data.len() >= 3 {
                    let tempo = (u32::from(data[0]) << 16)
                        | (u32::from(data[1]) << 8)
                        | u32::from(data[2]);
                    if tempo > 0 {
                        return 1_000_000.0 / f64::from(tempo);
                    }
                }
            }
        }
    }
    DEFAULT_TEMPO
}

/// Arrondit un tempo au battement par minute (`roundToBpm`).
fn round_to_bpm(beats_per_second: f64) -> f64 {
    (beats_per_second * 60.0).round() / 60.0
}

/// Pose les indications de tempo (`setTempo`).
pub fn set_tempo(
    score: &mut Score,
    midi: &MidiFile,
    _sigmap: &TimeSigMap,
    options: &ImportOptions,
) {
    let mut tempo_events: Vec<(Fraction, f64)> = Vec::new();
    for track in &midi.tracks {
        for (tick, raw) in &track.events {
            let EventKind::Meta { kind, data } = &raw.kind else {
                continue;
            };
            if *kind != meta::TEMPO || data.len() < 3 {
                continue;
            }
            let tempo = (u32::from(data[0]) << 16)
                | (u32::from(data[1]) << 8)
                | u32::from(data[2]);
            if tempo == 0 {
                continue;
            }
            let tick = to_muse_score_ticks(*tick, midi.division, midi.is_division_in_tps);
            tempo_events.push((tick, 1_000_000.0 / f64::from(tempo)));
        }
    }
    tempo_events.sort_by(|left, right| left.0.cmp(&right.0));

    let mut last_tempo = DEFAULT_TEMPO;
    for (tick, beats_per_second) in tempo_events {
        let beats_per_second = round_to_bpm(beats_per_second);
        if tick > Fraction::ZERO && (last_tempo - beats_per_second).abs() < 1e-9 {
            continue;
        }
        if !options.show_tempo_text
            && tick.is_zero()
            && (beats_per_second - DEFAULT_TEMPO).abs() < 1e-9
        {
            continue;
        }
        if !add_tempo(score, tick, beats_per_second, options.show_tempo_text) {
            continue;
        }
        last_tempo = beats_per_second;
    }
}

/// Pose un tempo au tick donné (`setTempoToScore`).
fn add_tempo(score: &mut Score, tick: Fraction, beats_per_second: f64, visible: bool) -> bool {
    let Some(measure_index) = score
        .measures
        .iter()
        .position(|measure| measure.start_tick <= tick && tick < measure.end_tick())
    else {
        return false;
    };
    let staff_count = score.staves.len();
    let measure = &mut score.measures[measure_index];
    let position = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind.is_chord_rest_type());
    let position = match position {
        Some(position) => position,
        None => {
            measure.add_segment(Segment::new(SegmentType::ChordRest, tick, staff_count));
            measure
                .segments
                .iter()
                .position(|segment| segment.tick == tick && segment.kind.is_chord_rest_type())
                .expect("segment créé")
        }
    };
    let bpm = (beats_per_second * 60.0).round() as i64;
    let mut tempo = Tempo::new(format!("<sym>metNoteQuarterUp</sym> = {bpm}"));
    tempo.beats_per_second = Some(beats_per_second);
    tempo.visible = visible;
    let segment = &mut score.measures[measure_index].segments[position];
    let staff_id = mscz2xml::model::StaffId::from_index(0);
    segment.add_annotation(Track::new(staff_id, 0), Annotation::Tempo(tempo));
    let _ = UNITS_PER_WHOLE;
    true
}
