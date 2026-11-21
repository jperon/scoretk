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

/// Transposition écrite d'un instrument GM (`InstrumentTemplate::transpose`).
///
/// Les gabarits de MuseScore portent un intervalle de transposition ; pour
/// l'import MIDI, le seul utilisé ici est celui de la contrebasse
/// (`share/instruments/instruments.xml`, instrument `contrabass` :
/// `<transposeDiatonic>-7</transposeDiatonic>`,
/// `<transposeChromatic>-12</transposeChromatic>`, programmes GM 32 et 43).
/// Un instrument non transpositeur garde `None`.
pub fn transpose_for_program(program: u8) -> Option<scoretk_core::model::Transpose> {
    match program {
        32 | 43 => Some(scoretk_core::model::Transpose::new(-7, -12)),
        _ => None,
    }
}
