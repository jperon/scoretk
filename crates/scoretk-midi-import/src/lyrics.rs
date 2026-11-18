// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_lyrics.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Paroles de l'import MIDI.
//!
//! Portage de `MidiLyrics::extractLyricsFromTrack` et `addLyricsToScore`
//! (`importmidi_lyrics.cpp`) : chaque méta-évènement de texte ou de parole est
//! rattaché à la note dont l'attaque d'origine correspond, puis posé sur
//! l'accord quantifié. Les titres `@T` (fichiers karaoké) ne sont pas repris :
//! les métadonnées d'œuvre passent par `processMeta`.

use scoretk_core::model::{Fraction, Lyrics, LyricsSyllabic, Score, SegmentItem, Track};

use super::chord::MTrack;

/// Préfixe des textes réservés (`META_PREFIX`).
const META_PREFIX: &str = "@";
/// Préfixe des titres (`TEXT_PREFIX`).
const TEXT_PREFIX: &str = "@T";

/// Vrai si le texte est un texte réservé (`isMetaText`).
pub fn is_meta_text(text: &str) -> bool {
    text.starts_with(META_PREFIX)
}

/// Vrai si le texte peut servir de parole (`isLyricText`).
pub fn is_lyric_text(text: &str) -> bool {
    !is_meta_text(text) || text.starts_with(TEXT_PREFIX)
}

/// Retire les barres obliques des fichiers karaoké (`removeSlashes`).
fn remove_slashes(text: &str) -> String {
    text.chars()
        .filter(|character| *character != '/' && *character != '\\')
        .collect()
}

/// Pose les paroles d'une piste (`addLyricsToScore`).
///
/// `lyrics` contient les couples `(attaque d'origine, texte)`. Une parole est
/// rattachée au premier accord dont une note porte cette attaque.
pub fn set_lyrics(score: &mut Score, track: &MTrack, lyrics: &[(Fraction, String)]) {
    let Some(staff) = track.staff else {
        return;
    };
    for (orig_tick, text) in lyrics {
        if !is_lyric_text(text) {
            continue;
        }
        let Some((on_time, chord)) = track.chords.iter().find(|(_, chord)| {
            chord
                .notes
                .iter()
                .any(|note| note.orig_on_time == *orig_tick)
        }) else {
            continue;
        };
        let voice = chord.voice;
        let Some(measure_index) = score
            .measures
            .iter()
            .position(|measure| measure.start_tick <= on_time && on_time < measure.end_tick())
        else {
            continue;
        };
        let track_id = Track::new(staff, voice as u8);
        let text = remove_slashes(text);
        for segment in &mut score.measures[measure_index].segments {
            if segment.tick != on_time {
                continue;
            }
            let Some(SegmentItem::ChordRest(chord_rest)) = segment.element_mut(track_id) else {
                continue;
            };
            chord_rest
                .lyrics
                .push(Lyrics::new(0, LyricsSyllabic::Single, text.clone()));
            break;
        }
    }
}

/// Fin de la pose des paroles (`MidiLyrics::setLyricsToScore`).
///
/// Les paroles ont déjà été posées par [`set_lyrics`] au fil du parcours des
/// méta-évènements : il ne reste rien à faire.
pub fn set_lyrics_to_score(_score: &mut Score, _tracks: &[MTrack]) {}
