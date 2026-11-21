// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `CompatMidiRender::renderGlissando`
// (`engraving/compat/midi/compatmidirender.cpp`), de `Glissando::pitchSteps`
// (`engraving/dom/glissando.cpp`) et de `EaseInOut::timeList`
// (`engraving/dom/easeInOut.cpp`) de MuseScore Studio, Copyright (C)
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

//! Course d'un glissando dans le rendu MIDI.
//!
//! `renderGlissando` (`compatmidirender.cpp:646-658`) appelle
//! `renderNoteArticulation` avec `Glissando::pitchSteps`
//! (`glissando.cpp:162-248`) et `requestedTicksPerNote = DIVISION`,
//! `fastestFreq = 16`, `slowestFreq = 0`. Le style par défaut est
//! `CHROMATIC` : la suite d'écarts est `0, 1, … |fin − début| − 1`.
//! `glissandoPart = 0.33` (`compatmidirender.cpp:795-870`) place la course
//! dans le dernier tiers de l'accord ; la dernière note tient jusqu'à la fin
//! de l'accord et l'arrivée est jouée par son propre accord.
//!
//! Les paramètres `easeIn`/`easeOut` du glissando ne sont pas modélisés
//! (`Pid::GLISS_EASEIN`/`GLISS_EASEOUT`, défaut 0) : la liste des départs est
//! linéaire, comme `EaseInOut::timeList` le fait quand les deux valent 0
//! (`easeInOut.cpp:140-154`).

use crate::repeat_list::DIVISION;

/// Évènements d'un glissando : `(décalage en ticks, hauteur, durée)`.
///
/// `span` est la durée totale de la chaîne de liaisons de la note de départ
/// (`totalTiedNoteTicks`), `chord_ticks` la durée effective de l'accord.
pub(crate) fn glissando(
    start: u8,
    end: u8,
    span: i64,
    chord_ticks: i64,
    beats_per_second: f64,
) -> Vec<(i64, u8, i64)> {
    if start == end || span <= 0 || chord_ticks <= 0 {
        return Vec::new();
    }
    let direction = if end > start { 1i32 } else { -1 };
    let count = i32::from(end).abs_diff(i32::from(start)) as i64;
    if count == 0 {
        return Vec::new();
    }
    let ticks_per_second = (beats_per_second * f64::from(DIVISION)).round() as i64;
    if ticks_per_second <= 0 {
        return Vec::new();
    }
    let min_ticks = (ticks_per_second / 16).max(1);
    let mut step = i64::from(DIVISION);
    if step < min_ticks {
        step = min_ticks;
    }
    // `renderNoteArticulation` (`compatmidirender.cpp:737-760`) : si la course
    // ne tient pas, le pas est divisé par le nombre de notes. `slowestFreq`
    // vaut 0 pour un glissando, donc le minimum n'est pas revérifié ensuite.
    if step * count > span {
        if step == min_ticks {
            return Vec::new();
        }
        step = span / count;
        if step <= 0 {
            return Vec::new();
        }
    }
    let milles = 1000 * step;
    let total = milles * count;
    let glissando_duration = (total as f64 * 0.33) as i64;
    let intervals = count - 1;
    if intervals <= 0 {
        return Vec::new();
    }
    // `timeList` avec `easeIn = easeOut = 0` : départs linéaires.
    let raw: Vec<i64> = (0..=intervals)
        .map(|index| ((index as f64 / intervals as f64) * glissando_duration as f64).round() as i64)
        .collect();
    // `compatmidirender.cpp:801-816` : la course est repoussée dans le dernier
    // tiers (`totalDuration - glissandoDuration - onTimes[1]`).
    let shift = total - glissando_duration - raw[1];
    let on: Vec<i64> = (0..=intervals)
        .map(|index| raw[index as usize] + if index > 0 { shift } else { 0 })
        .collect();

    // `sustainp` (`compatmidirender.cpp:846-848`) : la dernière note tient
    // jusqu'à la fin de l'accord.
    let sustain = 1000 * span - count * milles;
    let mut events = Vec::new();
    for index in 0..count as usize {
        let duration = if index + 1 < on.len() {
            on[index + 1] - on[index]
        } else {
            total - on[index] + sustain
        };
        // `makeEvent` puis `collectNote` (`compatmidirenderinternal.cpp:880`).
        let per_mille = on[index] / chord_ticks;
        let offset = chord_ticks * per_mille / 1000;
        let length = chord_ticks * (duration / chord_ticks) / 1000 - 1;
        if length > 0 {
            let pitch = (i32::from(start) + direction * index as i32).clamp(0, 127) as u8;
            events.push((offset, pitch, length));
        }
    }
    events
}
