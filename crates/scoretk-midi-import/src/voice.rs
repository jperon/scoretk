// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_voice.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. La
// séparation complète de MuseScore découpe les accords et les n-olets entre
// voix ; la version portée affecte les accords entiers à la première voix
// libre, puis trie les voix par hauteur moyenne, comme `sortVoices`.
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

//! Séparation des voix d'une portée.
//!
//! Portage de `MidiVoice::separateVoices` (`importmidi_voice.cpp`) : les
//! accords qui se chevauchent sont répartis sur plusieurs voix, puis les voix
//! sont renumérotées par hauteur moyenne décroissante (voix 1 = la plus aiguë),
//! comme `sortVoicesByPitch`.

use std::collections::BTreeMap;

use scoretk_core::model::{Fraction, VOICES};

use super::chord::MTrack;
use super::timesigmap::TimeSigMap;

/// Répartit les accords bruts en voix par chevauchement.
///
/// Équivalent de la répartition que `MidiTuplet::assignVoices` fait pendant la
/// détection des n-olets : la première voix libre (fin de note antérieure à
/// l'attaque) reçoit l'accord, sinon celle qui se libère le plus tôt. Sans
/// cette étape, toutes les couches rythmiques partageraient la voix 0 et la
/// détection des n-olets serait impossible.
pub fn assign_initial_voices(track: &mut MTrack, limit: usize) {
    assign_voices_impl(track, limit, None);
}

/// Variante après quantification : les fins de note sont alors sur la grille.
pub fn assign_initial_voices_quantized(track: &mut MTrack, limit: usize, basic_quant: Fraction) {
    assign_voices_impl(track, limit, Some(basic_quant));
}

fn assign_voices_impl(track: &mut MTrack, limit: usize, basic_quant: Option<Fraction>) {
    if track.drum_track || track.chords.is_empty() {
        return;
    }
    let limit = limit.clamp(1, VOICES);
    let keys = track.chords.keys();
    let mut voice_ends: Vec<Option<Fraction>> = vec![None; limit];
    for key in keys {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        let on_time = key.0;
        // `MidiTuplet::chordInterval` utilise `findMaxQuantizedOffTime` : une fin
        // de note à 44,542 tombe sur 44,5 et ne chevauche pas l'accord suivant
        // attaqué à 44,5. Comparer les fins brutes répartirait ces accords sur
        // deux voix et `minimizeNumberOfRests` allongerait ensuite la première
        // note d'un temps au lieu d'un croche (`chord_collect.mid`).
        let chord_end = match basic_quant {
            Some(bq) => chord
                .notes
                .iter()
                .map(|note| {
                    let quant = super::quant::quant_for_len(note.off_time - on_time, bq);
                    super::quant::quantize_value(note.off_time, quant)
                })
                .max()
                .unwrap_or(on_time),
            None => MTrack::max_off_time(chord),
        };
        let free = (0..limit).find(|voice| voice_ends[*voice].is_none_or(|end| end <= on_time));
        let voice = free.unwrap_or_else(|| {
            (0..limit)
                .min_by_key(|index| voice_ends[*index].unwrap_or(Fraction::ZERO))
                .unwrap_or(0)
        });
        voice_ends[voice] = Some(voice_ends[voice].map_or(chord_end, |end| end.max(chord_end)));
        if let Some(chord) = track.chords.get_mut(key) {
            chord.voice = voice;
        }
    }
}

/// Sépare les accords en voix (`MidiVoice::separateVoices`).
///
/// Renvoie vrai si au moins une voix a changé, ce qui déclenche la seconde
/// simplification des durées dans le pipeline.
pub fn separate_voices(
    tracks: &mut [MTrack],
    _sigmap: &TimeSigMap,
    max_voice_count: super::operations::VoiceCount,
) -> bool {
    let limit = max_voice_count.limit().min(VOICES);
    let mut changed = false;

    for track in tracks {
        if track.drum_track || track.chords.is_empty() {
            continue;
        }
        let bar_count = track
            .chords
            .iter()
            .map(|(_, chord)| chord.bar_index)
            .max()
            .unwrap_or(0)
            + 1;

        // Un seul passage pour répartir les clés par mesure : le code initial
        // parcourait toute la table pour chaque mesure (quadratique). La table
        // sparse évite aussi d'allouer pour un index de mesure aberrant.
        let mut keys_by_bar: BTreeMap<i32, Vec<_>> = BTreeMap::new();
        for (key, chord) in track.chords.iter_with_keys() {
            keys_by_bar.entry(chord.bar_index).or_default().push(*key);
        }

        for bar_index in 0..bar_count {
            let Some(keys) = keys_by_bar.get(&bar_index) else {
                continue;
            };
            if keys.is_empty() {
                continue;
            }

            let mut voice_ends: Vec<Option<Fraction>> = vec![None; limit];
            let mut voice_pitches: Vec<(i64, i64)> = vec![(0, 0); limit];
            for key in keys {
                let Some(chord) = track.chords.get(*key) else {
                    continue;
                };
                let chord_end = MTrack::max_off_time(chord);
                let on_time = key.0;
                let free =
                    (0..limit).find(|voice| voice_ends[*voice].is_none_or(|end| end <= on_time));
                let voice = free.unwrap_or_else(|| {
                    (0..limit)
                        .min_by_key(|index| voice_ends[*index].unwrap_or(Fraction::ZERO))
                        .unwrap_or(0)
                });
                let average = chord
                    .notes
                    .iter()
                    .map(|note| i64::from(note.pitch))
                    .sum::<i64>()
                    .checked_div(chord.notes.len() as i64)
                    .unwrap_or(0);
                voice_pitches[voice].0 += average * chord.notes.len() as i64;
                voice_pitches[voice].1 += chord.notes.len() as i64;
                voice_ends[voice] =
                    Some(voice_ends[voice].map_or(chord_end, |end| end.max(chord_end)));
                if let Some(chord) = track.chords.get_mut(*key) {
                    if chord.voice != voice {
                        changed = true;
                        chord.voice = voice;
                    }
                }
            }

            // `sortVoicesByPitch` : voix 0 = hauteur moyenne la plus haute.
            let mut averages: Vec<(i64, usize)> = voice_pitches
                .iter()
                .enumerate()
                .filter(|(_, (_, count))| *count > 0)
                .map(|(voice, (sum, count))| (sum / count, voice))
                .collect();
            averages.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
            let mut mapping = vec![0_usize; limit];
            for (new_voice, (_, old_voice)) in averages.iter().enumerate() {
                mapping[*old_voice] = new_voice;
            }
            for key in keys {
                if let Some(chord) = track.chords.get_mut(*key) {
                    chord.voice = mapping.get(chord.voice).copied().unwrap_or(0);
                }
            }
        }

        // Les n-olets suivent la voix de leurs accords.
        let tuplet_voices: Vec<(super::tuplet::TupletKey, usize)> = track
            .tuplets
            .keys()
            .map(|key| {
                let voice = track
                    .chords
                    .iter()
                    .find(|(on_time, _)| {
                        *on_time >= key.0 && *on_time < key.0 + Fraction::new(1, 64)
                    })
                    .map(|(_, chord)| chord.voice)
                    .unwrap_or(0);
                (*key, voice)
            })
            .collect();
        for (key, voice) in tuplet_voices {
            if let Some(tuplet) = track.tuplets.get_mut(&key) {
                tuplet.voice = voice;
            }
        }
    }

    changed
}
