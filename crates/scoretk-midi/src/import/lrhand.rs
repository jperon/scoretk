// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_lrhand.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. La
// programmation dynamique qui choisit la répartition note par note entre les
// deux mains est remplacée par une découpe par écart de hauteur, documentée
// dans `docs/agents/midi.md`.
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

//! Séparation main droite / main gauche d'un piano.
//!
//! Portage de `LRHand::needToSplit` et `LRHand::splitIntoLeftRightHands`
//! (`importmidi_lrhand.cpp`). La décision de découper est celle du C++ (un
//! accord dépasse l'octave) ; la répartition des notes entre les deux portées
//! est simplifiée : les notes sous la médiane du plus grand écart de hauteur de
//! l'accord vont à la main gauche.

use super::chord::{is_grand_staff_program, MidiChord, MTrack};
use super::instrument::is_grand_staff;
use super::operations::ImportOptions;

/// Vrai si la piste doit être scindée en deux (`needToSplit`).
pub fn need_to_split(track: &MTrack) -> bool {
    if track.drum_track || !is_grand_staff_program(track.program) {
        return false;
    }
    let octave = 12;
    track.chords.iter().any(|(_, chord)| {
        let mut min_pitch = i32::MAX;
        let mut max_pitch = i32::MIN;
        for note in &chord.notes {
            min_pitch = min_pitch.min(note.pitch);
            max_pitch = max_pitch.max(note.pitch);
        }
        max_pitch - min_pitch > octave
    })
}

/// Décide quelles pistes doivent être scindées (`setLeftRightHandSplit`).
///
/// Une piste dont la suivante forme déjà un grand staff n'est pas scindée.
pub fn find_splits(tracks: &[MTrack], _options: &ImportOptions) -> Vec<bool> {
    let mut result = Vec::with_capacity(tracks.len());
    for (index, track) in tracks.iter().enumerate() {
        let already_grand_staff = index + 1 < tracks.len() && is_grand_staff(track, &tracks[index + 1]);
        result.push(!already_grand_staff && need_to_split(track));
    }
    result
}

/// Indice de séparation des notes d'un accord, de 0 à `notes.len()`.
///
/// Les notes les plus graves que l'indice vont à la main gauche. L'indice
/// retenu est la plus grande chute de hauteur entre deux notes consécutives
/// (notes triées par hauteur), en évitant de tout attribuer à une seule main
/// quand l'accord est déjà réparti.
fn split_point(chord: &MidiChord, notes: &[i32]) -> usize {
    if notes.len() < 2 {
        let average = notes.first().copied().unwrap_or(60);
        return if average < 58 { notes.len() } else { 0 };
    }
    let mut best_index = notes.len() / 2;
    let mut best_gap = -1;
    for index in 1..notes.len() {
        let gap = notes[index] - notes[index - 1];
        if gap > best_gap {
            best_gap = gap;
            best_index = index;
        }
    }
    let average: i32 = notes.iter().sum::<i32>() / notes.len() as i32;
    if average < 58 {
        notes.len()
    } else if average > 64 && best_gap < 5 {
        0
    } else {
        best_index
    }
    .min(chord.notes.len())
}

/// Scinde une piste en main gauche et main droite
/// (`LRHand::splitIntoLeftRightHands`).
pub fn split_into_left_right_hands(tracks: &mut Vec<MTrack>, splits: &[bool]) {
    let mut result: Vec<MTrack> = Vec::with_capacity(tracks.len() + splits.iter().filter(|flag| **flag).count());
    for (index, track) in tracks.drain(..).enumerate() {
        if !splits.get(index).copied().unwrap_or(false) || track.chords.is_empty() {
            result.push(track);
            continue;
        }
        let mut right = track.clone();
        let mut left = track.clone();
        right.chords = super::chord::ChordMap::new();
        left.chords = super::chord::ChordMap::new();
        for (on_time, chord) in track.chords.iter() {
            let mut sorted: Vec<_> = chord.notes.clone();
            sorted.sort_by_key(|note| note.pitch);
            let pitches: Vec<i32> = sorted.iter().map(|note| note.pitch).collect();
            let point = split_point(chord, &pitches);
            if point == 0 {
                right.chords.insert(*on_time, chord.clone());
                continue;
            }
            if point == sorted.len() {
                left.chords.insert(*on_time, chord.clone());
                continue;
            }
            let mut right_chord = chord.clone();
            right_chord.notes = sorted[point..].to_vec();
            let mut left_chord = chord.clone();
            left_chord.notes = sorted[..point].to_vec();
            right.chords.insert(*on_time, right_chord);
            left.chords.insert(*on_time, left_chord);
        }
        right.had_initial_notes = true;
        left.had_initial_notes = true;
        result.push(right);
        result.push(left);
    }
    *tracks = result;
}
