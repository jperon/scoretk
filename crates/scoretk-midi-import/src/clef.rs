// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_clef.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. La
// programmation dynamique qui choisit les changements de clef en cours de
// portée n'est pas portée : seule la clef principale, déduite de la hauteur
// moyenne, est posée.
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

//! Clefs de l'import MIDI : clef principale d'après la hauteur moyenne.
//!
//! Portage de `MidiClef::createMainClefFromAveragePitch`,
//! `clefTypeFromAveragePitch` et `createClef` (`importmidi_clef.cpp`). Les
//! changements de clef (programmation dynamique `makeDynamicProgrammingStep`)
//! restent hors périmètre.

use scoretk_core::model::{Clef, ClefType, Score};

use super::chord::MTrack;
use super::instrument;
use super::operations::ImportOptions;

/// Hauteur médiane séparant la clef de sol de la clef de fa
/// (`clefMidPitch`).
pub fn clef_mid_pitch() -> i32 {
    60
}

/// Clef déduite d'une hauteur moyenne (`clefTypeFromAveragePitch`).
pub fn clef_type_from_average_pitch(average_pitch: i32) -> ClefType {
    if average_pitch < clef_mid_pitch() {
        ClefType::F
    } else {
        ClefType::G
    }
}

/// Hauteur moyenne des notes d'une piste (`AveragePitch`).
fn average_pitch(track: &MTrack) -> Option<i32> {
    let mut sum = 0_i64;
    let mut count = 0_i64;
    for (_, chord) in track.chords.iter() {
        for note in &chord.notes {
            sum += i64::from(note.pitch);
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some((sum as f64 / count as f64).round() as i32)
    }
}

/// Pose les clefs (`MidiClef::createClefs`).
///
/// La clef principale n'est recalculée que si le gabarit n'a pas fixé les
/// clefs (`!hasInstrument`) ou alterne sol et fa (`hasGFclefs`) ; sinon la
/// clef de concert du gabarit (guitare, contrebasse) est conservée, y compris
/// son octaviation. La programmation dynamique des changements en cours de
/// portée n'est pas portée : `changeClef` n'a donc pas d'effet observable.
pub fn create_clefs(score: &mut Score, tracks: &[MTrack], options: &ImportOptions) {
    let _ = options;
    for track in tracks {
        let Some(staff_id) = track.staff else {
            continue;
        };
        let has_instrument = track.ms_instr.is_some();
        if track.drum_track && !has_instrument {
            if let Some(staff) = score.staves.get_mut(staff_id.index()) {
                staff.default_clef = Clef::new(ClefType::Perc);
            }
            continue;
        }
        let can_change_clef = !has_instrument
            || track.ms_instr.is_some_and(instrument::has_gf_clefs);
        if !can_change_clef {
            continue;
        }
        let clef = match average_pitch(track) {
            Some(pitch) => clef_type_from_average_pitch(pitch),
            None => ClefType::G,
        };
        if let Some(staff) = score.staves.get_mut(staff_id.index()) {
            staff.default_clef = Clef::new(clef);
        }
    }
}
