// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_instrument.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. Le
// catalogue des instruments est la table générée
// `scoretk-core/src/instruments.rs` (ordre de `instruments.xml`).
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

//! Instruments de l'import MIDI : noms, grand staff, sélection du gabarit.
//!
//! Portage de `importmidi_instrument.cpp` : `instrumentName`, `isGrandStaff`,
//! `concatenateWithComma`, `setGrandStaffProgram`, `getPreferredInstrument`,
//! `findInstrumentsForProgram`, `findClosestInstrument`,
//! `sortInstrumentTemplates` et `findSuitableInstruments`. Le candidat retenu
//! est l'index 0 de `msInstrIndex` (valeur par défaut de `findInstrumentsForAllTracks`).

use std::collections::BTreeSet;

use scoretk_core::instruments::{preferred_template_for_program, Template, TEMPLATES};

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

/// Vrai si deux pistes partagent le canal de sortie (`isSameChannel`).
pub fn is_same_channel(first: &MTrack, second: &MTrack) -> bool {
    first.out_channel == second.out_channel
}

/// Vrai pour un orgue à trois portées (`is3StaffOrgan`) : programmes 16 à 20.
pub fn is_3_staff_organ(program: u8) -> bool {
    (16..=20).contains(&program)
}

/// Vrai si les deux pistes suivantes forment un grand staff
/// (`areNext2GrandStaff`).
pub fn are_next_2_grand_staff(index: usize, tracks: &[MTrack]) -> bool {
    index + 1 < tracks.len() && is_grand_staff(&tracks[index], &tracks[index + 1])
}

/// Vrai si les deux pistes suivantes forment la partie manuelle d'un orgue à
/// trois portées (`areNext3OrganStaff`).
pub fn are_next_3_organ_staff(index: usize, tracks: &[MTrack]) -> bool {
    if index + 2 >= tracks.len() {
        return false;
    }
    is_3_staff_organ(tracks[index].program)
        && is_grand_staff(&tracks[index], &tracks[index + 1])
        && is_same_channel(&tracks[index + 1], &tracks[index + 2])
}

/// Concatène deux noms avec une virgule (`concatenateWithComma`).
pub fn concatenate_with_comma(left: &str, right: &str) -> String {
    match (left.is_empty(), right.is_empty()) {
        (true, _) => right.to_owned(),
        (_, true) => left.to_owned(),
        _ => format!("{left}, {right}"),
    }
}

/// Propage le programme de la première portée d'un grand staff, et nomme les
/// portées d'un orgue à trois claviers (`setGrandStaffProgram`).
pub fn set_grand_staff_program(tracks: &mut [MTrack]) {
    let mut index = 0;
    while index < tracks.len() {
        if are_next_3_organ_staff(index, tracks) {
            let program = tracks[index].program;
            tracks[index + 1].program = program;
            tracks[index + 2].program = program;
            // Un seul nom près de l'accolade : « Manual » / « Pedal ».
            tracks[index].name = concatenate_with_comma(&tracks[index].name, "Manual");
            tracks[index + 1].name.clear();
            let pedal = concatenate_with_comma(&tracks[index + 2].name, "Pedal");
            tracks[index + 2].name = pedal;
            index += 3;
        } else if are_next_2_grand_staff(index, tracks) {
            let program = tracks[index].program;
            tracks[index + 1].program = program;
            if tracks[index].name != tracks[index + 1].name {
                // Un seul emplacement de nom près de l'accolade : les deux
                // noms sont vidés, le nom d'instrument servira.
                tracks[index].name.clear();
                tracks[index + 1].name.clear();
            }
            index += 2;
        } else {
            index += 1;
        }
    }
}

/// Toutes les hauteurs d'une piste (`findAllPitches`).
fn find_all_pitches(track: &MTrack) -> BTreeSet<i32> {
    let mut pitches = BTreeSet::new();
    for (_, chord) in track.chords.iter() {
        for note in &chord.notes {
            pitches.insert(note.pitch);
        }
    }
    pitches
}

/// Hauteurs non vides d'un jeu de percussion (`findNotEmptyDrumPitches`).
fn find_not_empty_drum_pitches(template: &Template) -> BTreeSet<i32> {
    (0..128)
        .filter(|pitch| template.drum_pitch_mask & (1u128 << pitch) != 0)
        .collect()
}

/// Vrai si une hauteur de la piste n'est pas définie par le jeu
/// (`hasNotDefinedDrumPitch`).
fn has_not_defined_drum_pitch(track_pitches: &BTreeSet<i32>, drum_pitches: &BTreeSet<i32>) -> bool {
    track_pitches
        .iter()
        .any(|pitch| !drum_pitches.contains(pitch))
}

/// Premier gabarit d'un groupe portant l'identifiant donné (`findInstrument`).
fn find_instrument(group_id: &str, instr_id: &str) -> Option<&'static Template> {
    TEMPLATES
        .iter()
        .find(|template| template.group == group_id && template.id == instr_id)
}

/// Nombre de hauteurs de `p1` absentes de `p2` (`countMissingPitches`).
fn count_missing_pitches(p1: &BTreeSet<i32>, p2: &BTreeSet<i32>) -> usize {
    p1.difference(p2).count()
}

/// Instrument de programme MIDI maximal inférieur à celui de la piste, le
/// plus proche des hauteurs jouées (`findClosestInstrument`).
fn find_closest_instrument(
    program: u8,
    pitches: &BTreeSet<i32>,
    is_drum_track: bool,
) -> Option<&'static Template> {
    let mut max_less_program: i32 = -1;
    let mut min_missing_pitches = i32::MAX;
    let mut closest_template: Option<&'static Template> = None;

    for template in TEMPLATES {
        if template.staff_group == "TAB" {
            continue;
        }
        if is_drum_track != template.use_drumset {
            continue;
        }

        if is_drum_track {
            let mut missing_pitches = i32::MAX;
            if template.has_drumset {
                let drum_pitches = find_not_empty_drum_pitches(template);
                missing_pitches = count_missing_pitches(pitches, &drum_pitches) as i32;
            }

            for &channel_program in template.channel_programs {
                if i32::from(channel_program) < i32::from(program)
                    && missing_pitches < min_missing_pitches
                {
                    max_less_program = i32::from(channel_program);
                    min_missing_pitches = missing_pitches;
                    closest_template = Some(template);
                    break;
                }
            }

            continue;
        }

        for &channel_program in template.channel_programs {
            if i32::from(channel_program) < i32::from(program)
                && i32::from(channel_program) > max_less_program
            {
                max_less_program = i32::from(channel_program);
                closest_template = Some(template);
                break;
            }
        }
    }
    closest_template
}

/// Gabarits candidats pour le programme d'une piste
/// (`findInstrumentsForProgram`), avant tri.
fn find_instruments_for_program(track: &MTrack) -> Vec<&'static Template> {
    let mut suitable_templates: Vec<&'static Template> = Vec::new();
    let program = track.program;
    let is_drum_track = track.drum_track;

    let preferred = if is_drum_track {
        None
    } else {
        preferred_template_for_program(program)
    };
    if let Some(preferred) = preferred {
        suitable_templates.push(preferred);
    }

    let track_pitches = if is_drum_track {
        find_all_pitches(track)
    } else {
        BTreeSet::new()
    };

    for template in TEMPLATES {
        if preferred.is_some_and(|preferred| template.id == preferred.id) {
            continue;
        }
        if template.staff_group == "TAB" {
            continue;
        }
        if is_drum_track != template.use_drumset {
            continue;
        }

        if is_drum_track {
            let drum_pitches = if template.has_drumset {
                find_not_empty_drum_pitches(template)
            } else {
                BTreeSet::new()
            };

            let mut suitable = false;
            for &channel_program in template.channel_programs {
                if channel_program == program {
                    // Un jeu explicite qui ne couvre pas les hauteurs de la
                    // piste disqualifie le gabarit (`break` du C++).
                    if template.has_drumset
                        && has_not_defined_drum_pitch(&track_pitches, &drum_pitches)
                    {
                        break;
                    }
                    suitable = true;
                    break;
                }
            }
            if suitable {
                suitable_templates.push(template);
            }

            continue;
        }

        if template.channel_programs.contains(&program) {
            suitable_templates.push(template);
        }
    }

    if suitable_templates.is_empty() {
        // Aucun gabarit MuseScore ne porte le programme : repli de MuseScore.
        if (80..=103).contains(&program) {
            let instr = if is_drum_track {
                find_instrument("electronic-instruments", "percussion-synthesizer")
            } else {
                find_instrument("electronic-instruments", "effect-synth")
            };
            if let Some(instr) = instr {
                suitable_templates.push(instr); // synthé générique
            }
        } else if program >= 112 {
            // Percussion sur une portée d'une ligne.
            if let Some(instr) = find_instrument("unpitched-percussion", "snare-drum") {
                suitable_templates.push(instr);
            }
        } else {
            if let Some(instr) = find_closest_instrument(program, &track_pitches, is_drum_track) {
                suitable_templates.push(instr);
            }
        }
    }

    suitable_templates
}

/// Hauteurs minimale et maximale de la piste (`findMinMaxPitch`).
fn find_min_max_pitch(track: &MTrack) -> (i32, i32) {
    let mut min_pitch = i32::MAX;
    let mut max_pitch = -1;
    for (_, chord) in track.chords.iter() {
        for note in &chord.notes {
            min_pitch = min_pitch.min(note.pitch);
            max_pitch = max_pitch.max(note.pitch);
        }
    }
    (min_pitch, max_pitch)
}

/// Écart entre la plage du gabarit et celle de la piste (`findMaxPitchDiff`).
///
/// Les deux plages sont comparées telles quelles, en hauteurs de la piste
/// (concert) : `minPitchP`/`maxPitchP` viennent du catalogue sans ajustement de
/// transposition, comme dans le C++.
fn find_max_pitch_diff(min_max_pitch: (i32, i32), template: &Template) -> i32 {
    let mut diff = 0;
    if min_max_pitch.0 < template.min_pitch {
        diff = template.min_pitch - min_max_pitch.0;
    }
    if min_max_pitch.1 > template.max_pitch {
        diff = diff.max(min_max_pitch.1 - template.max_pitch);
    }
    diff
}

/// Vrai si le gabarit appartient au genre « common » (`hasCommonGenre`).
fn has_common_genre(template: &Template) -> bool {
    template.genres.contains(&"common")
}

/// Tri stable des gabarits candidats (`sortInstrumentTemplates`) : écart de
/// plage, puis jeu de percussion explicite, puis genre « common », puis
/// nombre de genres décroissant. L'ordre du catalogue tranche les égalités.
fn sort_instrument_templates(templates: &mut [&'static Template], min_max_pitch: (i32, i32)) {
    templates.sort_by(|left, right| {
        let diff_left = find_max_pitch_diff(min_max_pitch, left);
        let diff_right = find_max_pitch_diff(min_max_pitch, right);
        if diff_left != diff_right {
            return diff_left.cmp(&diff_right);
        }
        // Un jeu de percussion explicite est préféré au jeu commun.
        if left.has_drumset != right.has_drumset {
            return if left.has_drumset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        // Le genre « common » est préféré.
        let common_left = has_common_genre(left);
        let common_right = has_common_genre(right);
        if common_left != common_right {
            return if common_left {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        right.genres.len().cmp(&left.genres.len())
    });
}

/// Gabarit d'instrument retenu pour une piste
/// (`findSuitableInstruments` puis `msInstrIndex` = 0, la valeur par défaut de
/// `findInstrumentsForAllTracks`).
pub fn template_for_track(track: &MTrack) -> Option<&'static Template> {
    let mut templates = find_instruments_for_program(track);
    if templates.is_empty() {
        return None;
    }
    let min_max_pitch = find_min_max_pitch(track);
    sort_instrument_templates(&mut templates, min_max_pitch);
    templates.first().copied()
}
