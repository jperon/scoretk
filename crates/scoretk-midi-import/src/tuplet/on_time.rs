// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est un portage de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, distribué sous GPL-3.0-only. Portage de
// `importmidi_tuplet.cpp` (`findOnTimeBetweenChords`), `modifié (réécrit en
// Rust) en 2026`.
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

//! `findOnTimeBetweenChords` : attaque d'un accord resserrée entre ses voisins.
//!
//! Portage de MuseScore 4.7.4 `importmidi_tuplet.cpp:957-1041`. Utilisé par
//! `chordInterval` (`importmidi_tuplet_voice.cpp:49-67`) pour décider si un
//! accord chevauche l'intervalle d'un n-olet lors de l'attribution des voix
//! (`MidiTuplet::assignVoices`). La grille de quantification est affinée tant
//! que l'attaque quantifiée n'est pas comprise strictement entre celles des
//! accords voisins de la même voix.

use scoretk_core::model::Fraction;

use super::{ChordKey, MTrack};
use crate::chord::min_allowed_duration;
use crate::quant::find_min_quantized_on_time;

/// Attaque resserrée d'un accord (`findOnTimeBetweenChords`).
///
/// Renvoie `bar_start` pour un accord situé avant la mesure ; sinon l'attaque
/// quantifiée la plus fine qui reste strictement après celle du précédent et
/// avant celle du suivant de la même voix, dans une fenêtre de `2 × grille`.
pub fn find_on_time_between_chords(
    track: &MTrack,
    key: ChordKey,
    basic_quant: Fraction,
    bar_start: Fraction,
) -> Fraction {
    if key.0 < bar_start {
        return bar_start;
    }
    let Some(chord) = track.chords.get(key) else {
        return key.0;
    };
    let voice = chord.voice;
    let keys = track.chords.keys();
    let Some(index) = keys.iter().position(|candidate| *candidate == key) else {
        return key.0;
    };
    let mut quant = basic_quant;
    let minimum = min_allowed_duration() * Fraction::from_integer(2);
    loop {
        let on_time = find_min_quantized_on_time(chord, key.0, quant);
        let mut changed = false;

        // Précédent accord de la même voix, dans la fenêtre `2 × grille`.
        let mut position = index;
        while position > 0 {
            position -= 1;
            let other = keys[position];
            if other.0 < key.0 && track.chords.get(other).map(|c| c.voice) == Some(voice) {
                let Some(previous) = track.chords.get(other) else {
                    break;
                };
                let previous_on_time = find_min_quantized_on_time(previous, other.0, quant);
                if on_time < previous_on_time {
                    if quant < minimum {
                        return on_time;
                    }
                    quant *= Fraction::HALF;
                    changed = true;
                }
                break;
            }
            if position == 0 || other.0 < key.0 - basic_quant * Fraction::from_integer(2) {
                break;
            }
        }

        // Suivant accord de la même voix, dans la fenêtre `2 × grille`.
        if !changed {
            let mut position = index + 1;
            while position < keys.len()
                && keys[position].0 < key.0 + basic_quant * Fraction::from_integer(2)
            {
                let other = keys[position];
                position += 1;
                if other.0 == key.0 || track.chords.get(other).map(|c| c.voice) != Some(voice) {
                    continue;
                }
                let Some(next) = track.chords.get(other) else {
                    break;
                };
                let next_on_time = find_min_quantized_on_time(next, other.0, quant);
                if on_time > next_on_time {
                    if quant < minimum {
                        return on_time;
                    }
                    quant *= Fraction::HALF;
                    changed = true;
                }
                break;
            }
        }

        if !changed {
            return on_time;
        }
    }
}
