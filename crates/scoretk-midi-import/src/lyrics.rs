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

/// Parole RP-017 prête à être posée sur un accord.
#[derive(Clone, Debug)]
pub struct LyricEvent {
    /// Attaque d'origine de la syllabe.
    pub tick: Fraction,
    /// Texte, sans le hyphen de syllabation ni les espaces de fin de mot.
    pub text: String,
    /// État de la syllabe dans son mot.
    pub syllabic: LyricsSyllabic,
    /// Durée du mélisme, quand des évènements vides prolongent la syllabe.
    pub melisma: Option<Fraction>,
}

/// Décode les conventions RP-017 d'une piste de méta-évènements de paroles.
///
/// Un évènement vide prolonge la syllabe précédente (mélisme) ; un évènement
/// dont le texte finit par un hyphen ouvre ou continue un mot, et le premier
/// évènement vide au tick 0 sert de drapeau « on » sans syllabe à prolonger.
/// MuseScore (`importmidi_lyrics.cpp`) ne décode aucune de ces conventions et
/// pose chaque texte en `LyricsSyllabic::Single` : le portage suit ici RP-017,
/// comme le demande la norme SMF.
pub fn parse_rp017(events: &[(Fraction, String)]) -> Vec<LyricEvent> {
    let mut result: Vec<LyricEvent> = Vec::new();
    let mut pending: Option<LyricsSyllabic> = None;
    let mut cursor = Fraction::ZERO;

    for (tick, raw) in events {
        let text = raw.trim_end_matches(' ').to_owned();
        if text.is_empty() {
            if let Some(previous) = result.last_mut() {
                if *tick > cursor {
                    let melisma = previous.melisma.get_or_insert(Fraction::ZERO);
                    *melisma += *tick - cursor;
                }
            }
            cursor = *tick;
            continue;
        }

        let (text, word_ends) = match text.strip_suffix('-') {
            Some(stem) => (stem.trim_end().to_owned(), false),
            None => (text, true),
        };
        let syllabic = if word_ends {
            match pending.take() {
                Some(LyricsSyllabic::Begin) | Some(LyricsSyllabic::Middle) => {
                    LyricsSyllabic::End
                }
                _ => LyricsSyllabic::Single,
            }
        } else {
            let continuing = matches!(pending, Some(LyricsSyllabic::Begin | LyricsSyllabic::Middle));
            let syllabic = if continuing {
                LyricsSyllabic::Middle
            } else {
                LyricsSyllabic::Begin
            };
            pending = Some(syllabic);
            syllabic
        };
        result.push(LyricEvent {
            tick: *tick,
            text,
            syllabic,
            melisma: None,
        });
        cursor = *tick;
    }
    result
}

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
/// Une parole est rattachée au premier accord dont une note porte son attaque
/// d'origine, avec l'état de syllabation et le mélisme décodés par
/// [`parse_rp017`].
pub fn set_lyrics(score: &mut Score, track: &MTrack, lyrics: &[LyricEvent]) {
    let Some(staff) = track.staff else {
        return;
    };
    for event in lyrics {
        if !is_lyric_text(&event.text) {
            continue;
        }
        let orig_tick = &event.tick;
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
        let text = remove_slashes(&event.text);
        for segment in &mut score.measures[measure_index].segments {
            if segment.tick != on_time {
                continue;
            }
            let Some(SegmentItem::ChordRest(chord_rest)) = segment.element_mut(track_id) else {
                continue;
            };
            let mut lyric = Lyrics::new(0, event.syllabic, text.clone());
            lyric.melisma = event.melisma;
            chord_rest.lyrics.push(lyric);
            break;
        }
    }
}

/// Fin de la pose des paroles (`MidiLyrics::setLyricsToScore`).
///
/// Les paroles ont déjà été posées par [`set_lyrics`] au fil du parcours des
/// méta-évènements : il ne reste rien à faire.
pub fn set_lyrics_to_score(_score: &mut Score, _tracks: &[MTrack]) {}
