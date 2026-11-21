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
//! Portage de `LRHand::needToSplit`, `LRHand::findSplits` et
//! `LRHand::splitIntoLeftRightHands` (`importmidi_lrhand.cpp`). La décision de
//! découper est celle du C++ (un accord dépasse l'octave) et la répartition
//! des notes utilise la programmation dynamique de `findSplits`
//! (`:318-403`), pas un seuil de hauteur fixe.

use super::chord::{is_grand_staff_program, MTrack, MidiChord};
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
pub fn find_splits(tracks: &[MTrack], options: &ImportOptions) -> Vec<bool> {
    let mut result = Vec::with_capacity(tracks.len());
    for (index, track) in tracks.iter().enumerate() {
        if !options.allow_staff_split_redefinition {
            // `setLeftRightHandSplit` (`importmidi.cpp:1076-1099`) n'est pas
            // appelé quand l'option est verrouillée (`canRedefineDefaultLater`
            // faux) : la valeur par défaut s'applique alors à toutes les
            // pistes, exactement comme les tests amont `voiceSeparation`.
            result.push(options.do_staff_split && need_to_split(track));
            continue;
        }
        let already_grand_staff =
            index + 1 < tracks.len() && is_grand_staff(track, &tracks[index + 1]);
        result.push(!already_grand_staff && need_to_split(track));
    }
    result
}

/// Un choix de répartition pour un accord (`LRHand::SplitTry`).
#[derive(Clone, Debug)]
struct SplitTry {
    penalty: i32,
    prev_split_point: usize,
}

/// Données d'un accord dans la programmation dynamique (`ChordSplitData`).
#[derive(Clone, Debug)]
struct ChordSplitData {
    key: crate::chord::ChordKey,
    on_time: Fraction,
    notes: Vec<crate::chord::MidiNote>,
    possible_splits: Vec<SplitTry>,
}

use super::chord::{ChordKey, MidiNote};
use scoretk_core::model::Fraction;

/// `LRHand::isOctave` (`importmidi_lrhand.cpp:180-188`).
fn is_octave(notes: &[MidiNote], beg: usize, end: usize) -> bool {
    end - beg == 2 && notes[end - 1].pitch - notes[beg].pitch == 12
}

/// `LRHand::findPitchWidthPenalty` (`:158-178`).
fn find_pitch_width_penalty(notes: &[MidiNote], split_point: usize) -> i32 {
    let mut penalty = 0;
    if split_point > 0 {
        let width = (notes[0].pitch - notes[split_point - 1].pitch).abs();
        penalty += if width <= 12 {
            0
        } else if width <= 14 {
            20
        } else {
            100
        };
    }
    if split_point < notes.len() {
        let width = (notes[split_point].pitch - notes[notes.len() - 1].pitch).abs();
        penalty += if width <= 12 {
            0
        } else if width <= 14 {
            20
        } else {
            100
        };
    }
    penalty
}

/// `LRHand::findSimilarityPenalty` (`:190-219`).
fn find_similarity_penalty(
    notes: &[MidiNote],
    prev: &[MidiNote],
    split: usize,
    prev_split: usize,
) -> i32 {
    let mut penalty = 0;
    if split > 0 && prev_split > 0 {
        let low = is_octave(notes, 0, split);
        let prev_low = is_octave(prev, 0, prev_split);
        if low && prev_low {
            penalty -= 12;
        } else if split > 1 && prev_split > 1 {
            penalty -= 5;
        }
    }
    if split < notes.len() && prev_split < prev.len() {
        let high = is_octave(notes, split, notes.len());
        let prev_high = is_octave(prev, prev_split, prev.len());
        if high && prev_high {
            penalty -= 12;
        } else if notes.len() - split > 1 && prev.len() - prev_split > 1 {
            penalty -= 5;
        }
    }
    if split == 1 && prev_split == 1 {
        penalty -= 12;
    }
    if notes.len() - split == 1 && prev.len() - prev_split == 1 {
        penalty -= 12;
    }
    penalty
}

/// `LRHand::areOffTimesEqual` (`:221-235`).
fn are_off_times_equal(notes: &[MidiNote], beg: usize, end: usize) -> bool {
    let first = notes[beg].off_time;
    notes[beg..end].iter().all(|note| note.off_time == first)
}

/// `LRHand::findDurationPenalty` (`:237-245`).
fn find_duration_penalty(notes: &[MidiNote], split: usize) -> i32 {
    let mut penalty = 0;
    if split > 0 && are_off_times_equal(notes, 0, split) {
        penalty -= 10;
    }
    if split < notes.len() && are_off_times_equal(notes, split, notes.len()) {
        penalty -= 10;
    }
    penalty
}

/// `LRHand::findNoteCountPenalty` (`:247-257`).
fn find_note_count_penalty(notes: &[MidiNote], split: usize) -> i32 {
    let left = split;
    let right = notes.len() - split;
    if right > 0 && left > 0 && left < right {
        5
    } else if right == 0 && left > 1 {
        10
    } else {
        0
    }
}

/// `LRHand::findIntersectionPenalty` (`:259-316`).
fn find_intersection_penalty(
    current_on_time: Fraction,
    mut pos: usize,
    mut split: usize,
    max_chord_len: Fraction,
    splits: &[ChordSplitData],
    has_low_notes: bool,
    has_high_notes: bool,
) -> i32 {
    let mut penalty = 0;
    while splits[pos].on_time + max_chord_len > current_on_time {
        let notes = &splits[pos].notes;
        if has_low_notes
            && split > 0
            && notes[..split]
                .iter()
                .any(|note| note.off_time > current_on_time)
        {
            penalty += 10;
        }
        if has_high_notes
            && split < notes.len()
            && notes[split..]
                .iter()
                .any(|note| note.off_time > current_on_time)
        {
            penalty += 10;
        }
        if pos == 0 {
            break;
        }
        split = splits[pos].possible_splits[split].prev_split_point;
        pos -= 1;
    }
    penalty
}

/// `LRHand::findSplits` (`importmidi_lrhand.cpp:318-403`) : programmation
/// dynamique qui choisit, pour chaque accord, le point de séparation entre
/// main gauche (notes basses) et main droite.
fn find_chord_splits(keys: &[ChordKey], track: &MTrack) -> Vec<ChordSplitData> {
    let mut splits: Vec<ChordSplitData> = Vec::with_capacity(keys.len());
    let mut max_chord_len = Fraction::ZERO;
    for (pos, key) in keys.iter().enumerate() {
        let Some(chord) = track.chords.get(*key) else {
            continue;
        };
        let mut notes = chord.notes.clone();
        notes.sort_by_key(|note| note.pitch);
        let len = notes
            .iter()
            .map(|note| note.off_time)
            .max()
            .unwrap_or(key.0)
            - key.0;
        if len > max_chord_len {
            max_chord_len = len;
        }
        let mut data = ChordSplitData {
            key: *key,
            on_time: key.0,
            notes,
            possible_splits: Vec::new(),
        };
        for split in 0..=data.notes.len() {
            let mut try_data = SplitTry {
                penalty: find_pitch_width_penalty(&data.notes, split)
                    + find_duration_penalty(&data.notes, split)
                    + find_note_count_penalty(&data.notes, split),
                prev_split_point: 0,
            };
            if pos > 0 {
                let prev = &splits[pos - 1].notes;
                let mut best = None;
                for prev_split in 0..=prev.len() {
                    let prior = splits[pos - 1].possible_splits[prev_split].penalty
                        + find_similarity_penalty(&data.notes, prev, split, prev_split)
                        + find_intersection_penalty(
                            key.0,
                            pos - 1,
                            prev_split,
                            max_chord_len,
                            &splits,
                            split > 0,
                            split < data.notes.len(),
                        );
                    if best.is_none_or(|(min, _): (i32, usize)| prior < min) {
                        best = Some((prior, prev_split));
                    }
                }
                if let Some((min, prev_split)) = best {
                    try_data.penalty += min;
                    try_data.prev_split_point = prev_split;
                }
            }
            data.possible_splits.push(try_data);
        }
        splits.push(data);
    }
    splits
}

/// `LRHand::splitChords` (`:327-365`) : remonte la programmation dynamique
/// pour répartir les notes entre les deux mains.
fn split_chords(splits: &[ChordSplitData], track: &mut MTrack) -> Vec<(ChordKey, MidiChord)> {
    if splits.is_empty() {
        return Vec::new();
    }
    let last = splits.len() - 1;
    let mut split = splits[last]
        .possible_splits
        .iter()
        .enumerate()
        .min_by_key(|(_, try_data)| try_data.penalty)
        .map_or(0, |(index, _)| index);
    let mut left = Vec::new();
    for pos in (0..=last).rev() {
        let data = &splits[pos];
        let Some(mut chord) = track.chords.get(data.key).cloned() else {
            if pos > 0 {
                split = data.possible_splits[split.min(data.possible_splits.len() - 1)]
                    .prev_split_point;
            }
            continue;
        };
        if split > 0 {
            let notes = data.notes.clone();
            if split < notes.len() {
                let mut left_chord = chord.clone();
                left_chord.notes = notes[..split].to_vec();
                left.push((data.key, left_chord));
                chord.notes = notes[split..].to_vec();
                if let Some(target) = track.chords.get_mut(data.key) {
                    *target = chord;
                }
            } else {
                left.push((data.key, chord.clone()));
                track.chords.remove(data.key);
            }
        }
        if pos > 0 && split < data.possible_splits.len() {
            split = data.possible_splits[split].prev_split_point;
        }
    }
    left
}

/// Scinde une piste en main gauche et main droite
/// (`LRHand::splitIntoLeftRightHands`, `importmidi_lrhand.cpp:427-443`).
pub fn split_into_left_right_hands(tracks: &mut Vec<MTrack>, splits: &[bool]) {
    let mut result: Vec<MTrack> =
        Vec::with_capacity(tracks.len() + splits.iter().filter(|flag| **flag).count());
    for (index, track) in tracks.drain(..).enumerate() {
        if !splits.get(index).copied().unwrap_or(false) || track.chords.is_empty() {
            result.push(track);
            continue;
        }
        let keys = track.chords.keys();
        let chord_splits = find_chord_splits(&keys, &track);
        let mut right = track.clone();
        let left_chords = split_chords(&chord_splits, &mut right);
        let mut left = track.clone();
        left.chords = super::chord::ChordMap::new();
        for (key, chord) in left_chords {
            left.chords.insert_at(key, chord);
        }
        right.had_initial_notes = true;
        left.had_initial_notes = true;
        result.push(right);
        result.push(left);
    }
    *tracks = result;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::MidiNote;
    use scoretk_core::model::Fraction;

    fn chord(notes: &[(i32, Fraction)]) -> MidiChord {
        MidiChord {
            notes: notes
                .iter()
                .map(|(pitch, off)| MidiNote::new(*pitch, 80, *off, Fraction::ZERO))
                .collect(),
            ..MidiChord::default()
        }
    }

    /// Réduction de `SaintJosephQuiLessive.midi` : une note seule sous la
    /// médiane (55) suivie de deux accords plus larges. MuseScore 4.7.4
    /// (`LRHand::findSplits`, `importmidi_lrhand.cpp:318-403`) la garde dans
    /// la main droite ; notre seuil fixe de 58 l'envoyait à la main gauche.
    #[test]
    fn note_seule_sous_la_mediane_reste_a_droite() {
        let mut track = MTrack::default();
        track
            .chords
            .insert(Fraction::ZERO, chord(&[(55, Fraction::new(1, 8))]));
        track.chords.insert(
            Fraction::new(1, 8),
            chord(&[(60, Fraction::new(1, 4)), (64, Fraction::new(1, 4))]),
        );
        track.chords.insert(
            Fraction::new(1, 4),
            chord(&[(60, Fraction::new(3, 8)), (64, Fraction::new(3, 8))]),
        );
        // Un accord large déclenche `needToSplit`, comme dans le fichier réel.
        track.chords.insert(
            Fraction::new(3, 8),
            chord(&[(48, Fraction::new(1, 2)), (70, Fraction::new(1, 2))]),
        );
        let mut tracks = vec![track];
        let splits = find_splits(&tracks, &ImportOptions::default());
        split_into_left_right_hands(&mut tracks, &splits);
        let right = &tracks[0];
        assert!(
            right
                .chords
                .iter()
                .any(|(_, chord)| chord.notes.iter().any(|note| note.pitch == 55)),
            "la note 55 doit rester dans la main droite : {tracks:?}"
        );
    }
    /// `LRHand::splitStaff` ne crée pas de portée vide quand la
    /// programmation dynamique garde toutes les notes à droite.
    #[test]
    fn pas_de_portee_gauche_vide() {
        let mut track = MTrack::default();
        track.chords.insert(
            Fraction::ZERO,
            chord(&[(60, Fraction::new(1, 2)), (77, Fraction::new(1, 2))]),
        );
        let mut tracks = vec![track];
        let splits = find_splits(&tracks, &ImportOptions::default());
        split_into_left_right_hands(&mut tracks, &splits);
        assert!(
            tracks.iter().all(|track| !track.chords.is_empty()),
            "aucune portée vide ne doit être créée : {tracks:?}"
        );
    }

    /// `LRHand::splitChords`: un point de séparation à 0 ne crée aucun
    /// accord pour la main gauche (pas de portée vide, `importmidi_lrhand.cpp:411-417`).
    #[test]
    fn separation_sans_accord_a_gauche() {
        let mut track = MTrack::default();
        let key = track
            .chords
            .insert(Fraction::ZERO, chord(&[(60, Fraction::new(1, 2))]));
        let splits = vec![ChordSplitData {
            key,
            on_time: Fraction::ZERO,
            notes: track.chords.get(key).unwrap().notes.clone(),
            possible_splits: vec![
                SplitTry {
                    penalty: -10,
                    prev_split_point: 0,
                },
                SplitTry {
                    penalty: 0,
                    prev_split_point: 0,
                },
            ],
        }];
        assert!(split_chords(&splits, &mut track).is_empty());
        assert_eq!(track.chords.len(), 1);
    }
}
