// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_instrument.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. Le
// catalogue complet des instruments de MuseScore n'est pas embarqué : les
// noms viennent de la table General MIDI (`generalmidi.cpp`) et les noms
// courts du catalogue ne sont pas repris.
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

//! Instruments de l'import MIDI : noms, grand staff, programmes.
//!
//! Portage partiel de `MidiInstr` : `instrumentName` (nom General MIDI ou
//! « Percussion »), `isGrandStaff`, `concatenateWithComma` et
//! `setGrandStaffProgram`. La recherche dans le catalogue MuseScore
//! (`findSuitableInstruments`) n'est pas portée : elle ne sert qu'à fournir un
//! nom court et une transposition, absents d'un fichier MIDI.

use super::chord::{is_grand_staff_program, MTrack};

mod gm_names;
pub use gm_names::{get_midi_name, GM1_NAMES};

/// Nom du programme (`MidiInstr::instrumentName`).
pub fn instrument_name(program: u8, is_drum_track: bool) -> String {
    if is_drum_track {
        "Percussion".to_owned()
    } else {
        get_midi_name(program).to_owned()
    }
}

/// Nom d'instrument d'une piste.
pub fn track_instrument_name(track: &MTrack) -> String {
    instrument_name(track.program, track.drum_track)
}

/// Vrai si deux pistes forment un grand staff (`MidiInstr::isGrandStaff`).
pub fn is_grand_staff(first: &MTrack, second: &MTrack) -> bool {
    first.out_channel == second.out_channel
        && is_grand_staff_program(first.program)
        && !first.drum_track
        && !second.drum_track
}

/// Concatène deux noms avec une virgule (`concatenateWithComma`).
pub fn concatenate_with_comma(left: &str, right: &str) -> String {
    match (left.is_empty(), right.is_empty()) {
        (true, _) => right.to_owned(),
        (_, true) => left.to_owned(),
        _ => format!("{left}, {right}"),
    }
}

/// Propage le programme de la première portée d'un grand staff
/// (`setGrandStaffProgram`).
pub fn set_grand_staff_program(tracks: &mut [MTrack]) {
    let mut index = 0;
    while index + 1 < tracks.len() {
        if is_grand_staff(&tracks[index], &tracks[index + 1]) {
            tracks[index + 1].program = tracks[index].program;
            index += 2;
        } else {
            index += 1;
        }
    }
}

/// Choisit le gabarit d'instrument d'une piste, comme
/// `MidiInstr::findInstrumentsForProgram` + `sortInstrumentTemplates`
/// (`importmidi_instrument.cpp:409-566`). Le premier candidat est retenu,
/// comme le `msInstrIndex` par défaut de MuseScore.
pub fn template_for_track(track: &MTrack) -> Option<&'static scoretk_core::instruments::Template> {
    use scoretk_core::instruments::{preferred_template_for_program, Template, TEMPLATES};
    let program = i16::from(track.program);
    let drum = track.drum_track;
    let preferred = if drum {
        None
    } else {
        preferred_template_for_program(track.program)
    };
    let mut candidates: Vec<&'static Template> = Vec::new();
    if let Some(pref) = preferred {
        candidates.push(pref);
    }
    for template in TEMPLATES {
        if preferred.is_some_and(|pref| pref.id == template.id) {
            continue;
        }
        if template.staff_group == "TAB" || template.use_drumset != drum {
            continue;
        }
        if template.programs.contains(&program) {
            candidates.push(template);
        }
    }
    if candidates.is_empty() {
        // `findClosestInstrument` : plus grand programme inférieur à celui de
        // la piste, hors tablatures et avec le même type de jeu.
        let mut best: Option<(i16, &'static Template)> = None;
        for template in TEMPLATES {
            if template.staff_group == "TAB" || template.use_drumset != drum {
                continue;
            }
            for candidate_program in template.programs.iter().copied() {
                if candidate_program < program
                    && best.is_none_or(|(current, _)| candidate_program > current)
                {
                    best = Some((candidate_program, template));
                }
            }
        }
        if let Some((_, template)) = best {
            candidates.push(template);
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let (min_pitch, max_pitch) = track
        .chords
        .iter()
        .flat_map(|(_, chord)| chord.notes.iter().map(|note| note.pitch))
        .fold((i32::MAX, i32::MIN), |(low, high), pitch| {
            (low.min(pitch), high.max(pitch))
        });
    candidates.sort_by(|left, right| {
        let diff = |template: &&'static scoretk_core::instruments::Template| -> i32 {
            if min_pitch == i32::MAX {
                return 0;
            }
            let mut value = 0;
            if template.min_pitch > 0 && min_pitch < template.min_pitch {
                value = template.min_pitch - min_pitch;
            }
            if template.max_pitch > 0 && max_pitch > template.max_pitch {
                value = value.max(max_pitch - template.max_pitch);
            }
            value
        };
        diff(left).cmp(&diff(right))
    });
    candidates.first().copied()
}
