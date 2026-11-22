// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_drum.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. La table
// de batterie standard de MuseScore (`smDrumset`, chargée depuis
// `share/instruments/instruments.xml`) n'est pas embarquée : seules les
// hauteurs de voix 1 du kit standard (grosse caisse et charleston pédale) sont
// reprises, et les noms de fûts ne sont pas traduits.
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

//! Percussion : répartition en voix, découpe des accords, accolades.
//!
//! Portage de `MidiDrum` (`importmidi_drum.cpp`) : la percussion s'écrit sur
//! une ou deux voix selon la direction de hampe de la table de batterie. Sans
//! la table complète, la répartition se limite aux hauteurs connues pour être
//! hampes en bas, et la découpe par hauteur (`splitDrumTracks`) n'est exercée
//! que si l'option `doStaffSplit` est demandée (elle ne l'est pas par défaut).

use scoretk_core::model::{Score, StaffBracket};

use super::chord::{min_allowed_duration, MTrack};

/// Hauteurs jouées en voix 1 (hampe vers le bas) du kit standard de MuseScore.
///
/// Grosse caisse 2 (35), grosse caisse 1 (36) et charleston pédale (44) sont
/// les seules entrées `<voice>1</voice>` du kit `drumset` de
/// `share/instruments/instruments.xml`.
const VOICE_1_PITCHES: &[i32] = &[35, 36, 44];

/// Vrai si la hauteur est écrite en voix 1.
pub fn is_voice_1(pitch: i32) -> bool {
    VOICE_1_PITCHES.contains(&pitch)
}

/// Sépare en deux voix les notes de percussion (`splitDrumVoices`).
pub fn split_drum_voices(tracks: &mut [MTrack]) {
    for track in tracks {
        if !track.drum_track || track.chords.is_empty() {
            continue;
        }
        let keys = track.chords.keys();
        for key in keys {
            let move_notes: Vec<bool> = track
                .chords
                .get(key)
                .map(|chord| {
                    chord
                        .notes
                        .iter()
                        .map(|note| is_voice_1(note.pitch))
                        .collect()
                })
                .unwrap_or_default();
            if !move_notes.iter().any(|move_note| *move_note) {
                continue;
            }
            let Some(chord) = track.chords.get(key) else {
                continue;
            };
            let mut new_chord = chord.clone();
            new_chord.voice = 1;
            new_chord.notes.clear();
            let mut kept = Vec::new();
            for (index, note) in chord.notes.iter().enumerate() {
                if move_notes.get(index).copied().unwrap_or(false) {
                    new_chord.notes.push(note.clone());
                } else {
                    kept.push(note.clone());
                }
            }
            if !new_chord.notes.is_empty() {
                track.chords.insert(key.0, new_chord);
            }
            if let Some(chord) = track.chords.get_mut(key) {
                chord.notes = kept;
            }
            if track
                .chords
                .get(key)
                .is_some_and(|chord| chord.notes.is_empty())
            {
                track.chords.remove(key);
            }
        }
    }
}

/// Découpe éventuelle des pistes de percussion par hauteur
/// (`splitDrumTracks`).
///
/// L'option `doStaffSplit` n'est jamais posée par défaut sur une piste de
/// percussion : la fonction ne fait donc rien tant qu'un appelant ne la
/// demande pas. Elle reste portée pour la structure du pipeline.
pub fn split_drum_tracks(tracks: &mut [MTrack]) {
    for track in tracks {
        if !track.drum_track || track.chords.is_empty() {
            continue;
        }
        // `opers.doStaffSplit` vaut faux par défaut : aucune découpe.
    }
}

/// Recherche les n-olets d'une piste de percussion
/// (`findAllTupletsForDrums`).
///
/// MuseScore détecte les n-olets séparément par voix puis fusionne les
/// accords ; la détection portée ici travaille directement sur la piste, ce
/// qui donne le même résultat pour les groupes d'une seule voix.
pub fn find_all_tuplets_for_drums(
    track: &mut MTrack,
    sigmap: &super::timesigmap::TimeSigMap,
    basic_quant: scoretk_core::model::Fraction,
    options: &super::operations::ImportOptions,
    is_human: bool,
) {
    super::tuplet::find_all_tuplets(track, sigmap, basic_quant, options, is_human);
    let _ = min_allowed_duration();
}

/// Découpe les accords de percussion à durées inégales
/// (`splitUnequalChords` s'applique aussi aux percussions).
pub fn split_unequal_drum_chords(tracks: &mut [MTrack]) {
    // La découpe des accords inégaux est commune à toutes les pistes ; elle est
    // appliquée après la séparation des voix dans `convert_midi`.
    let _ = tracks;
}

/// Pose une accolade sur les groupes de portées de percussion
/// (`setStaffBracketForDrums`).
pub fn set_staff_bracket_for_drums(score: &mut Score, tracks: &[MTrack]) {
    let mut start: Option<usize> = None;
    let mut count = 0;
    let flush = |score: &mut Score, start: Option<usize>, count: usize| {
        if let Some(staff) = start {
            if count > 1 {
                if let Some(model_staff) = score.staves.get_mut(staff) {
                    model_staff.brackets.push(StaffBracket {
                        kind: 0,
                        span: count,
                        visible: true,
                    });
                }
            }
        }
    };
    for track in tracks {
        if track.drum_track {
            if start.is_none() {
                start = track.staff.map(|staff| staff.index());
            }
            count += 1;
        } else {
            flush(score, start, count);
            start = None;
            count = 0;
        }
    }
    flush(score, start, count);
}
