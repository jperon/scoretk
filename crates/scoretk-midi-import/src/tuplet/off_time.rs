// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est un portage de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, distribué sous GPL-3.0-only. Portage de
// `importmidi_tuplet.cpp` (`minimizeOffTimeError`), `modifié (réécrit en
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

//! `minimizeOffTimeError` : notes trop longues sorties du premier accord d'un
//! n-olet.
//!
//! Portage de MuseScore 4.7.4 `importmidi_tuplet.cpp:442-503`, appelé par
//! `findTuplets` juste après `splitFirstTupletChords` et seulement si
//! `tupletVoiceLimit() > 1`.
//!
//! Un accord peut contenir, sur la même attaque, les notes de plusieurs
//! n-olets **et** d'une voix ordinaire (`tuplet_2_voices_tuplet_non` : 67
//! dans le triolet, 62 dans la voix 2, tous deux à 0,0). Pour chaque note du
//! premier accord d'un n-olet, si elle est plus longue qu'une note du n-olet
//! mais tient dans le n-olet, on compare son erreur de fin quantifiée sur la
//! grille du n-olet et sur la grille régulière : si la grille régulière fait
//! moins d'erreur, la note **sort** du n-olet et rejoint les accords hors
//! n-olet, où `setNonTupletVoices` lui donnera une autre voix. Le reste du
//! premier accord est inséré comme nouvel accord à la même date et devient le
//! premier accord du n-olet ; si plus aucune note ne reste, l'accord est
//! retiré (et le n-olet avec lui s'il n'a plus d'accord).

use scoretk_core::model::Fraction;

use super::tuplet_limits;
use super::TupletInfo;
use crate::chord::{ChordKey, MTrack};
use crate::quant::{find_off_time_quant_error, find_off_time_tuplet_quant_error};

/// `minimizeOffTimeError` (`importmidi_tuplet.cpp:454-503`).
pub fn minimize_off_time_error(
    track: &mut MTrack,
    tuplets: &mut Vec<TupletInfo>,
    non_tuplets: &mut Vec<ChordKey>,
    start_bar_tick: Fraction,
    basic_quant: Fraction,
) {
    let mut index = 0;
    while index < tuplets.len() {
        if tuplets[index].chords.is_empty() || tuplets[index].first_chord_index != 0 {
            index += 1;
            continue;
        }
        let key = tuplets[index].chords[0];
        let on_time = key.0.max(start_bar_tick);
        let Some(original) = track.chords.get(key).cloned() else {
            index += 1;
            continue;
        };
        let tuplet_number = i64::from(tuplets[index].tuplet_number.max(1));
        let tuplet_note_len = tuplets[index].len / Fraction::from_integer(tuplet_number);
        let tuplet_len = tuplets[index].len;
        let ratio = tuplet_limits(tuplets[index].tuplet_number).ratio;
        let mut removed: Vec<usize> = Vec::new();
        let mut leaved: Vec<usize> = Vec::new();
        for (position, note) in original.notes.iter().enumerate() {
            let length = note.off_time - on_time;
            if length <= tuplet_len && length > tuplet_note_len {
                // S'il ne resterait aucune note dans l'accord, le n-olet
                // deviendrait vide : on en garde toujours au moins une
                // (sauf accord à une seule note, qui peut être entièrement
                // retiré du n-olet).
                let garde = if tuplets[index].chords.len() == 1 {
                    original.notes.len() > removed.len()
                } else {
                    original.notes.len() > removed.len() + 1
                };
                if garde {
                    let tuplet_error = find_off_time_tuplet_quant_error(
                        on_time,
                        note.off_time,
                        tuplet_len,
                        ratio,
                        start_bar_tick,
                    );
                    let regular_error =
                        find_off_time_quant_error(on_time, &original, note.off_time, basic_quant);
                    if tuplet_error >= regular_error {
                        removed.push(position);
                        continue;
                    }
                }
            }
            leaved.push(position);
        }
        if !removed.is_empty() {
            let kept: Vec<_> = leaved
                .iter()
                .map(|position| original.notes[*position].clone())
                .collect();
            let moved: Vec<_> = removed
                .iter()
                .map(|position| original.notes[*position].clone())
                .collect();
            // L'accord d'origine garde les notes sorties et reste hors
            // n-olet ; le n-olet reçoit un nouvel accord avec les autres.
            let mut moved_chord = original.clone();
            moved_chord.notes = moved;
            track.chords.insert_at(key, moved_chord);
            non_tuplets.push(key);
            if kept.is_empty() {
                tuplets[index].chords.remove(0);
                if tuplets[index].chords.is_empty() {
                    tuplets.remove(index);
                    continue;
                }
            } else {
                let mut kept_chord = original;
                kept_chord.notes = kept;
                let new_key = track.chords.insert(key.0, kept_chord);
                tuplets[index].chords[0] = new_key;
            }
        }
        index += 1;
    }
}
