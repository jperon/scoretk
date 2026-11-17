// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/pausemap.cpp` de MuseScore Studio,
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

//! Pauses de section, rendues par des ticks et des changements de tempo.
//!
//! Un fichier MIDI ne peut pas contenir de silence de durée libre : MuseScore
//! insère donc une mesure de ticks supplémentaires à chaque pause et encadre
//! cette mesure par deux changements de tempo (`PauseMap` dans
//! `engraving/compat/midi/pausemap.cpp`). Les ticks insérés décalent tous les
//! évènements suivants ; [`PauseMap::tick_with_pauses`] effectue ce décalage.

use std::collections::BTreeMap;

use mscz2xml::model::{Score, TimeSig};

use super::repeat_list::{RepeatList, DIVISION};

/// Durée par défaut d'une pause de section, en secondes (`Sid::sectionPause`).
const SECTION_PAUSE_SECONDS: f64 = 3.0;

/// Pause source : tick MuseScore et durée en secondes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pause {
    tick: i64,
    seconds: f64,
}

/// Pause de section insérée dans la performance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PauseSpan {
    /// Tick d'exécution du début de pause **avant** décalage (tick déroulé,
    /// sans les pauses).
    pub base_utick: i64,
    /// Tick d'exécution du début de la pause, pauses précédentes comprises.
    pub paused_utick: i64,
    /// Ticks insérés par cette pause.
    pub inserted: i64,
    /// Durée de la pause, en secondes.
    pub seconds: f64,
}

/// Carte des ticks insérés et des changements de tempo associés.
#[derive(Clone, Debug, Default)]
pub(crate) struct PauseMap {
    /// Tick d'exécution du début de pause → total cumulé de ticks insérés.
    offsets: BTreeMap<i64, i64>,
    /// Changements de tempo (tick d'exécution, battements par seconde) dus aux
    /// pauses.
    pub(crate) tempo_events: Vec<(i64, f64)>,
    /// Pauses insérées, dans l'ordre de construction.
    spans: Vec<PauseSpan>,
}

impl PauseMap {
    /// Construit la carte à partir des pauses de section de la partition.
    pub(crate) fn build(
        score: &Score,
        repeat_list: &RepeatList,
        time_signatures: &[(i64, TimeSig)],
    ) -> Self {
        let pauses = section_pauses(score);
        let mut map = Self {
            offsets: BTreeMap::from([(0, 0)]),
            tempo_events: Vec::new(),
            spans: Vec::new(),
        };
        if pauses.is_empty() {
            return map;
        }

        for segment in &repeat_list.segments {
            let end = repeat_list.segment_end(segment);
            let offset = segment.utick - segment.tick;
            for pause in &pauses {
                if pause.tick < segment.tick || pause.tick > end || pause.tick == segment.tick {
                    continue;
                }
                let utick = pause.tick + offset;
                let before = map.offset_at(utick);
                let quarter_notes = quarter_notes_per_measure(time_signatures, pause.tick);
                let extra = (quarter_notes * f64::from(DIVISION)).round() as i64 + before;
                map.offsets.insert(utick, extra);
                map.spans.push(PauseSpan {
                    base_utick: utick,
                    paused_utick: utick + before,
                    inserted: extra - before,
                    seconds: pause.seconds,
                });

                // Tempo de la pause, puis retour au tempo en vigueur.
                let pause_tempo = quarter_notes / pause.seconds.max(f64::EPSILON);
                map.tempo_events.push((utick + before, pause_tempo));
                map.tempo_events
                    .push((utick + extra, tempo_at(score, pause.tick)));
            }
        }
        map
    }

    /// Total des ticks insérés avant `utick` (le tick de pause inclus).
    fn offset_at(&self, utick: i64) -> i64 {
        self.offsets
            .range(..=utick)
            .next_back()
            .map_or(0, |(_, offset)| *offset)
    }

    /// Tick d'exécution réel (`PauseMap::tickWithPauses`).
    pub(crate) fn tick_with_pauses(&self, utick: i64) -> i64 {
        utick + self.offset_at(utick)
    }

    /// Tick d'exécution réel, pauses **strictement** antérieures seulement.
    ///
    /// Sert aux fins de mesures : une pause qui ouvre la mesure suivante n'est
    /// pas comptée dans la fin de la mesure courante, ce qui évite de compter
    /// deux fois la pause à la frontière.
    pub(crate) fn tick_with_pauses_before(&self, utick: i64) -> i64 {
        utick + self.offset_at_before(utick)
    }

    /// Pauses de section insérées, dans l'ordre de construction.
    pub(crate) fn spans(&self) -> &[PauseSpan] {
        &self.spans
    }

    /// Total des ticks insérés strictement avant `utick`.
    fn offset_at_before(&self, utick: i64) -> i64 {
        self.offsets
            .range(..utick)
            .next_back()
            .map_or(0, |(_, offset)| *offset)
    }
}

/// Pauses de section : le saut porte sur la mesure suivante.
fn section_pauses(score: &Score) -> Vec<Pause> {
    let mut pauses = Vec::new();
    for (index, measure) in score.measures.iter().enumerate() {
        if index > 0 && measure.section_break_before {
            pauses.push(Pause {
                tick: measure
                    .start_tick
                    .ticks(super::repeat_list::UNITS_PER_WHOLE),
                seconds: SECTION_PAUSE_SECONDS,
            });
        }
    }
    pauses
}

/// Nombre de noires par mesure de l'indication de mesure en vigueur.
fn quarter_notes_per_measure(time_signatures: &[(i64, TimeSig)], tick: i64) -> f64 {
    let time_sig = time_signatures
        .iter()
        .rev()
        .find(|(at, _)| *at <= tick)
        .map_or(4.0, |(_, time_sig)| {
            if time_sig.denominator > 0 {
                4.0 * f64::from(time_sig.numerator) / f64::from(time_sig.denominator)
            } else {
                4.0
            }
        });
    time_sig.max(1.0)
}

/// Tempo en vigueur à un tick source, en battements par seconde.
fn tempo_at(score: &Score, tick: i64) -> f64 {
    let mut value = 2.0;
    for measure in &score.measures {
        let measure_tick = measure
            .start_tick
            .ticks(super::repeat_list::UNITS_PER_WHOLE);
        if measure_tick > tick {
            break;
        }
        for segment in &measure.segments {
            let segment_tick = segment.tick.ticks(super::repeat_list::UNITS_PER_WHOLE);
            if segment_tick > tick {
                continue;
            }
            for entry in &segment.annotations {
                if let mscz2xml::model::Annotation::Tempo(tempo) = &entry.annotation {
                    if let Some(beats_per_second) = tempo.beats_per_second {
                        value = beats_per_second;
                    }
                }
            }
        }
    }
    value
}
