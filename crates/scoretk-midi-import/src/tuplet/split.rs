// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est un portage de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, distribué sous GPL-3.0-only. Portage de
// `importmidi_tuplet.cpp` (`sortNotesByPitch`, `averagePitch`,
// `sortTupletsByAveragePitch`, `splitTupletChord`, `splitFirstTupletChords`),
// modifié (réécrit en Rust) en 2026.
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

//! `splitFirstTupletChords` : deux n-olets qui partagent leur premier accord.
//!
//! Portage de `importmidi_tuplet.cpp:224-249` (`sortNotesByPitch`,
//! `averagePitch`, `sortTupletsByAveragePitch`) et `:401-440`
//! (`splitTupletChord`, `splitFirstTupletChords`), appelés par `findTuplets`
//! dans cet ordre (`:1116-1119`) :
//!
//! 1. les notes de chaque accord de la mesure sont triées par hauteur ;
//! 2. les n-olets sont triés par hauteur moyenne décroissante ;
//! 3. quand deux n-olets commencent sur le **même** accord, celui-ci est
//!    scindé : le n-olet **antérieur** garde la première note (la plus
//!    grave), le suivant garde les autres.
//!
//! Sans les deux tris, le partage dépend de l'ordre de détection et non de la
//! hauteur : `tuplet_2_voices_3_5_tuplets` (test amont) recevait la note grave
//! dans le triolet et l'aiguë dans le quintuplet, donc les voix étaient
//! inversées dans la référence.

use scoretk_core::model::Fraction;

use super::TupletInfo;
use crate::chord::MTrack;

/// Trie les notes de chaque accord de la mesure par hauteur **décroissante**
/// (`sortNotesByPitch`, MuseScore 4.7.4 `importmidi_tuplet.cpp:224-238`).
///
/// Le comparateur de 4.7.4 est `n1.pitch > n2.pitch` (la plus aiguë en
/// premier) ; l'instantané 5.0.0 du dépôt amont a l'ordre inverse
/// (`n1.pitch < n2.pitch`). La référence de comportement étant 4.7.4, c'est
/// l'ordre décroissant qui est porté : `splitTupletChord` garde la **première**
/// note pour le n-olet antérieur, donc la plus aiguë, et la référence
/// `tuplet_2_voices_3_5_tuplets` (67 dans le triolet, 64 dans le quintuplet)
/// n'est reproduite qu'ainsi.
pub fn sort_notes_by_pitch(track: &mut MTrack, start: Fraction, end: Fraction) {
    let keys: Vec<_> = track.chords.range(start, end).map(|(key, _)| key).collect();
    for key in keys {
        if let Some(chord) = track.chords.get_mut(key) {
            chord
                .notes
                .sort_by_key(|note| std::cmp::Reverse(note.pitch));
        }
    }
}

/// Hauteur moyenne d'un n-olet, arrondie (`averagePitch`, `:204-222`).
///
/// La moyenne porte sur **toutes** les notes des accords du n-olet, y compris
/// l'accord partagé avec un autre n-olet, comme dans le C++.
pub fn average_pitch(track: &MTrack, tuplet: &TupletInfo) -> i32 {
    let mut sum = 0i32;
    let mut count = 0i32;
    for key in &tuplet.chords {
        let Some(chord) = track.chords.get(*key) else {
            continue;
        };
        for note in &chord.notes {
            sum += note.pitch;
            count += 1;
        }
    }
    if count == 0 {
        return -1;
    }
    // `qRound` : arrondi au plus proche, demi vers le haut.
    (f64::from(sum) / f64::from(count)).round() as i32
}

/// Trie les n-olets par hauteur moyenne **décroissante**
/// (`sortTupletsByAveragePitch`, `:240-249`).
pub fn sort_tuplets_by_average_pitch(track: &MTrack, tuplets: &mut [TupletInfo]) {
    tuplets.sort_by(|left, right| {
        average_pitch(track, right)
            .cmp(&average_pitch(track, left))
            .then(left.on_time.cmp(&right.on_time))
    });
}

/// Scinde le premier accord d'un n-olet en deux accords à la même date
/// (`splitTupletChord`, `:403-420`).
///
/// Le n-olet visé garde un accord d'**une** note (la première, donc la plus
/// grave après `sort_notes_by_pitch`) ; l'autre accord de même date conserve
/// les notes restantes et reste le premier accord du n-olet courant.
fn split_tuplet_chord(track: &mut MTrack, tuplets: &mut [TupletInfo], index: usize) {
    let Some(key) = tuplets[index].chords.first().copied() else {
        return;
    };
    let Some(original) = track.chords.get(key).cloned() else {
        return;
    };
    // Le C++ suppose au moins deux notes (assertion en fin de fonction).
    if original.notes.len() < 2 {
        return;
    }
    let mut first = original.clone();
    first.notes.truncate(1);
    let mut rest = original;
    rest.notes.remove(0);
    // Le nouvel accord (une note) est inséré à la même date et devient le
    // premier accord du n-olet antérieur ; l'ancien garde les autres notes.
    let new_key = track.chords.insert(key.0, first);
    track.chords.insert_at(key, rest);
    tuplets[index].chords[0] = new_key;
}

/// `splitFirstTupletChords` (`:422-440`).
///
/// Les n-olets sont supposés déjà triés par `sort_tuplets_by_average_pitch`.
/// Pour chaque n-olet, on cherche le **dernier** n-olet antérieur dont le
/// premier accord est le même et qui commence aussi sur ce premier accord
/// (`first_chord_index == 0`) ; c'est lui qui est scindé.
pub fn split_first_tuplet_chords(track: &mut MTrack, tuplets: &mut [TupletInfo]) {
    for now in 0..tuplets.len() {
        if tuplets[now].first_chord_index != 0 {
            continue;
        }
        let Some(key) = tuplets[now].chords.first().copied() else {
            continue;
        };
        let previous = (0..now).rev().find(|index| {
            tuplets[*index].first_chord_index == 0 && tuplets[*index].chords.first() == Some(&key)
        });
        if let Some(previous) = previous {
            split_tuplet_chord(track, tuplets, previous);
        }
    }
}
