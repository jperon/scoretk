// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_tuplet.cpp`, `importmidi_tuplet_detect.cpp` et
// `importmidi_tuplet_tonotes.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026. La
// détection est restreinte aux n-olets dont la durée de note n'est pas une
// figure binaire (3, 5, 7, 9) et aux n-olets simples, sans imbrication.
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

//! N-olets de l'import MIDI : limites, détection et rattachement des éléments.
//!
//! Portage de `MidiTuplet` : la table des ratios (`tupletsLimits`), la
//! détection par mesure (`detectTuplets`, simplifiée) et les fonctions de
//! recherche utilisées par la quantification et la création des notes.

use scoretk_core::model::Fraction;

use super::chord::{min_allowed_duration, MTrack};
use super::meter::{divisions_of_bar_for_tuplets, is_simple_note_duration};
use super::timesigmap::TimeSigMap;

/// Clé d'un n-olet dans [`MTrack::tuplets`] : date et rang.
pub type TupletKey = (Fraction, u64);

/// N-olet en cours d'import (`MidiTuplet::TupletData`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupletData {
    /// Voix du n-olet.
    pub voice: usize,
    /// Début du n-olet.
    pub on_time: Fraction,
    /// Durée totale.
    pub len: Fraction,
    /// Nombre de notes du n-olet (3, 5, 7, 9…).
    pub tuplet_number: i32,
    /// Nombre d'éléments rattachés (accords et silences).
    pub element_count: usize,
}

/// Limites d'un n-olet (`TupletLimits`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TupletLimits {
    /// Ratio de conversion, par exemple `3/2` pour un triolet.
    pub ratio: Fraction,
    /// Nombre minimal de notes.
    pub min_note_count: i32,
    /// Nombre minimal quand le n-olet sépare des voix.
    pub min_note_count_add_voice: i32,
    /// Nombre minimal pour des notes piquées.
    pub min_note_count_staccato: i32,
    /// Nombre minimal pour une performance humaine.
    pub min_note_count_human: i32,
}

/// Table des n-olets connus (`tupletsLimits`).
pub fn tuplet_limits(tuplet_number: i32) -> TupletLimits {
    let (numerator, denominator, min_note_count, add_voice, staccato, human) = match tuplet_number {
        2 => (2, 3, 1, 2, 2, 2),
        3 => (3, 2, 1, 3, 3, 2),
        4 => (4, 3, 3, 4, 3, 3),
        5 => (5, 4, 3, 4, 4, 4),
        7 => (7, 8, 4, 6, 5, 6),
        9 => (9, 8, 6, 7, 7, 8),
        other => {
            // Repli pour un nombre inconnu : ratio n / plus grande puissance de
            // deux inférieure, comme les valeurs de la table.
            let mut denominator = 1;
            while denominator * 2 < other {
                denominator *= 2;
            }
            (other, denominator, 1, 2, 2, 2)
        }
    };
    TupletLimits {
        ratio: Fraction::new(i64::from(numerator), i64::from(denominator)),
        min_note_count,
        min_note_count_add_voice: add_voice,
        min_note_count_staccato: staccato,
        min_note_count_human: human,
    }
}

/// N-olets contenus dans une durée, ramenés au début de la mesure
/// (`findTupletsInBarForDuration`).
pub fn find_tuplets_in_bar_for_duration(
    track: &MTrack,
    voice: usize,
    bar_start: Fraction,
    duration_on_time: Fraction,
    duration_len: Fraction,
) -> Vec<TupletData> {
    let mut result = Vec::new();
    for (on_time, tuplet) in &track.tuplets {
        let on_time = on_time.0;
        if on_time < bar_start {
            continue;
        }
        if on_time >= duration_on_time + duration_len {
            break;
        }
        if tuplet.voice == voice && duration_on_time < on_time + tuplet.len {
            let mut local = tuplet.clone();
            local.on_time -= bar_start;
            result.push(local);
        }
    }
    // Le C++ trie par longueur décroissante avant de construire la liste de
    // durées.
    result.sort_by_key(|tuplet| std::cmp::Reverse(tuplet.len));
    result
}

/// Retire les n-olets sans accord (`removeEmptyTuplets`).
pub fn remove_empty_tuplets(track: &mut MTrack) {
    let referenced: Vec<TupletKey> = track
        .chords
        .iter()
        .flat_map(|(_, chord)| {
            let mut keys = Vec::new();
            if let Some(key) = chord.tuplet {
                keys.push(key);
            }
            for note in &chord.notes {
                if let Some(key) = note.tuplet {
                    keys.push(key);
                }
            }
            keys
        })
        .collect();
    track.tuplets.retain(|key, _| referenced.contains(key));
}

/// Cherche les n-olets d'une piste (`MidiTuplet::findAllTuplets`).
///
/// La détection parcourt les divisions métriques de chaque mesure et accepte
/// un groupe de `n` accords également répartis quand la durée d'une note du
/// groupe n'est pas une figure binaire : c'est le cas des triolets, quintolet,
/// septolet et nonolet. Les duolets et quadruolets, dont la grille est binaire,
/// ne sont pas détectés (options `search2plets`/`search4plets` neutralisées).
pub fn find_all_tuplets(track: &mut MTrack, sigmap: &TimeSigMap, basic_quant: Fraction) {
    if track.chords.is_empty() {
        return;
    }
    let bar_count = track
        .chords
        .iter()
        .map(|(_, chord)| chord.bar_index)
        .max()
        .unwrap_or(-1)
        + 1;
    let mut next_seq = 0_u64;

    for bar_index in 0..bar_count {
        let bar_start =
            super::timesigmap::fraction_from_ticks(sigmap.bar2tick(i64::from(bar_index), 0));
        let bar_len = sigmap
            .timesig(sigmap.bar2tick(i64::from(bar_index), 0))
            .measure_len();
        let mut spans = divisions_of_bar_for_tuplets(bar_len);
        spans.retain(|span| !span.is_zero());
        spans.sort_by(|left, right| right.cmp(left));

        for span in spans {
            let beat = super::meter::beat_length(bar_len);
            let step = if beat.is_positive() {
                span.min(beat)
            } else {
                span
            };
            // Garde de progression : un fichier MIDI mal formé (mesure de
            // longueur nulle ou division invalide) ne doit pas bloquer la
            // boucle. `span` est déjà filtré non nul, mais une mesure
            // incohérente peut rendre `step` non positif.
            if !step.is_positive() {
                continue;
            }
            let mut start = bar_start;
            while start + span <= bar_start + bar_len {
                if let Some((voice, tuplet_number, tolerance)) =
                    detect_tuplet(track, bar_index, start, span, basic_quant)
                {
                    let key = (start, next_seq);
                    next_seq += 1;
                    track.tuplets.insert(
                        key,
                        TupletData {
                            voice,
                            on_time: start,
                            len: span,
                            tuplet_number,
                            element_count: 0,
                        },
                    );
                    mark_tuplet(track, key, start, span, tolerance);
                }
                start += step;
            }
        }
    }
    remove_empty_tuplets(track);
    let _ = basic_quant;
}

/// Cherche un n-olet de `n` notes également réparties dans un intervalle.
///
/// Renvoie la voix, le nombre de notes et la tolérance retenue pour le
/// marquage. La détection cherche un accord sur chacun des `n` points d'une
/// grille `span / n` ; les accords étrangers au n-olet peuvent s'intercaler
/// (couche rythmique suivante, note piquée) à condition d'être plus courts que
/// la case — c'est le cas de `MidiTuplet::detectTuplets`, qui accepte les
/// notes de n-olet piquées et les sépare ensuite en voix. Une note finale plus
/// courte que sa case est allongée au marquage (`applyTupletStaccato`).
fn detect_tuplet(
    track: &MTrack,
    bar_index: i32,
    start: Fraction,
    span: Fraction,
    basic_quant: Fraction,
) -> Option<(usize, i32, Fraction)> {
    let _ = basic_quant;
    // Ne parcourt que les accords de l'intervalle, comme le C++ qui limite la
    // recherche à la mesure courante. Un `iter()` complet coûtait O(nombre total
    // d'accords) à chaque position candidate (quadratique sur les fichiers
    // denses).
    let chords: Vec<(Fraction, usize, Fraction, bool)> = track
        .chords
        .range(start, start + span)
        .filter(|(_, chord)| chord.bar_index == bar_index)
        .map(|((on_time, _), chord)| {
            (
                on_time,
                chord.voice,
                MTrack::max_off_time(chord),
                chord.is_in_tuplet,
            )
        })
        .collect();
    if chords.len() < 3 {
        return None;
    }

    for tuplet_number in [3, 5, 7, 9] {
        let n = tuplet_number as usize;
        let grid = span / Fraction::from_integer(i64::from(tuplet_number));
        if is_simple_note_duration(grid) {
            continue;
        }
        let tolerance =
            (grid * Fraction::new(1, 8)).min(min_allowed_duration() * Fraction::from_integer(2));

        let mut matched: Vec<usize> = Vec::with_capacity(n);
        let mut ok = true;
        for k in 0..n {
            let expected = start + grid * Fraction::from_integer(k as i64);
            let found = chords
                .iter()
                .enumerate()
                .filter(|(_, (on_time, _, _, is_in_tuplet))| {
                    !*is_in_tuplet && (*on_time - expected).abs() <= tolerance
                })
                .min_by(|left, right| {
                    (left.1 .0 - expected)
                        .abs()
                        .cmp(&(right.1 .0 - expected).abs())
                })
                .map(|(index, _)| index);
            match found {
                Some(index) if !matched.contains(&index) => matched.push(index),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }

        // Les notes du n-olet ne dépassent pas leur case (à la tolérance près) ;
        // les accords étrangers sont plus courts qu'une case.
        let main_line = matched.iter().all(|index| {
            let (on_time, _, off_time, _) = chords[*index];
            off_time - on_time <= grid + tolerance
        });
        let foreign_are_short = chords
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched.contains(index))
            .all(|(_, (on_time, _, off_time, _))| *off_time - *on_time < grid - tolerance);
        if !main_line || !foreign_are_short {
            continue;
        }
        // Aucun accord étranger entre deux attaques du n-olet : une couche
        // rythmique intercalée entre les notes rendrait la grille ambiguë
        // (c'est le cas d'une suite de 32e qui contient les points d'un
        // triolet). Les accords étrangers ne peuvent apparaître qu'après la
        // dernière attaque, comme la couche suivante de `testTuplets8`.
        let mut onsets: Vec<Fraction> = matched.iter().map(|index| chords[*index].0).collect();
        onsets.sort();
        let foreign_between = chords
            .iter()
            .enumerate()
            .any(|(index, (on_time, _, _, _))| {
                if matched.contains(&index) {
                    return false;
                }
                onsets.windows(2).any(|window| {
                    *on_time > window[0] + tolerance && *on_time < window[1] - tolerance
                })
            });
        if foreign_between {
            continue;
        }
        let voice = matched.first().map(|index| chords[*index].1).unwrap_or(0);
        let same_voice = matched.iter().all(|index| chords[*index].1 == voice);
        if !same_voice {
            continue;
        }
        return Some((voice, tuplet_number, tolerance));
    }
    None
}

/// Marque les accords et notes d'un n-olet.
fn mark_tuplet(
    track: &mut MTrack,
    key: TupletKey,
    start: Fraction,
    span: Fraction,
    tolerance: Fraction,
) {
    let Some(tuplet) = track.tuplets.get(&key).cloned() else {
        return;
    };
    let n = i64::from(tuplet.tuplet_number);
    let grid = span / Fraction::from_integer(n);
    let mut matched: Vec<TupletKey> = Vec::new();
    for k in 0..n {
        let expected = start + grid * Fraction::from_integer(k);
        let found = track
            .chords
            .keys()
            .into_iter()
            .filter(|chord_key| {
                (chord_key.0 - expected).abs() <= tolerance
                    && track
                        .chords
                        .get(*chord_key)
                        .is_some_and(|chord| chord.voice == tuplet.voice && !chord.is_in_tuplet)
            })
            .min_by(|left, right| (left.0 - expected).abs().cmp(&(right.0 - expected).abs()));
        if let Some(found) = found {
            if !matched.contains(&found) {
                matched.push(found);
            }
        }
    }
    let count = matched.len();
    for chord_key in matched {
        let cell_end =
            start + grid * Fraction::from_integer(((chord_key.0 - start) / grid).numerator() + 1);
        if let Some(chord) = track.chords.get_mut(chord_key) {
            chord.is_in_tuplet = true;
            chord.tuplet = Some(key);
            for note in &mut chord.notes {
                if note.off_time < cell_end && note.off_time <= start + span {
                    // `applyTupletStaccato` : une note de n-olet trop courte est
                    // allongée à sa case.
                    note.off_time = cell_end.min(start + span);
                }
                if note.off_time <= start + span + tolerance {
                    note.is_in_tuplet = true;
                    note.tuplet = Some(key);
                }
            }
        }
    }
    if let Some(tuplet) = track.tuplets.get_mut(&key) {
        tuplet.element_count = count;
    }
}

/// Vrai si un accord ou silence appartient entièrement à un n-olet
/// (`addElementToTuplet`).
pub fn tuplet_for_element(
    track: &MTrack,
    voice: usize,
    on_time: Fraction,
    len: Fraction,
) -> Option<TupletKey> {
    track
        .tuplets
        .iter()
        .find(|(_, tuplet)| {
            tuplet.voice == voice
                && on_time >= tuplet.on_time
                && on_time + len <= tuplet.on_time + tuplet.len
        })
        .map(|(key, _)| *key)
}
