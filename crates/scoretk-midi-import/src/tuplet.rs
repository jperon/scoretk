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

use super::chord::{min_allowed_duration, ChordKey, MTrack};
use super::meter::{divisions_of_bar_for_tuplets_raw, is_simple_note_duration};
use super::timesigmap::{fraction_from_ticks, TimeSigMap};

mod detect;
mod filter;
mod off_time;
pub(crate) mod on_time;
mod split;
pub mod voice;

/// N-olet candidat pendant la détection (`MidiTuplet::TupletInfo`).
#[derive(Clone, Debug, Default)]
pub struct TupletInfo {
    /// Nombre de notes du n-olet (3, 5, 7, 9, 2 ou 4).
    pub tuplet_number: i32,
    /// Début du n-olet.
    pub on_time: Fraction,
    /// Durée totale.
    pub len: Fraction,
    /// Accords retenus, triés par date.
    pub chords: Vec<ChordKey>,
    /// Indice de la position du premier accord.
    pub first_chord_index: i32,
    /// Somme des erreurs de n-olet.
    pub tuplet_sum_error: Fraction,
    /// Somme des erreurs de quantification régulière.
    pub regular_sum_error: Fraction,
    /// Somme des silences internes.
    pub sum_length_of_rests: Fraction,
    /// Indices des notes piquées : accord → indice de note.
    pub staccato_chords: std::collections::BTreeMap<ChordKey, usize>,
    /// Rang du candidat dans la mesure.
    pub id: usize,
}

/// Détecte les n-olets d'une piste sans la modifier (`detectTuplets`).
pub fn detect_all_tuplets(
    track: &MTrack,
    sigmap: &TimeSigMap,
    basic_quant: Fraction,
    options: &super::operations::ImportOptions,
    is_human: bool,
) -> Vec<TupletInfo> {
    if track.chords.is_empty() {
        return Vec::new();
    }
    let keys = track.chords.keys();
    let bar_count = track
        .chords
        .iter()
        .map(|(_, chord)| chord.bar_index)
        .max()
        .unwrap_or(-1)
        + 1;
    let detect_options = detect::DetectOptions {
        search_2plets: options.search_2plets,
        search_3plets: options.search_3plets,
        search_4plets: options.search_4plets,
        search_5plets: options.search_5plets,
        search_7plets: options.search_7plets,
        search_9plets: options.search_9plets,
        is_human,
        simplify_durations: options.simplify_durations,
    };
    let mut candidates = Vec::new();
    for bar_index in 0..bar_count {
        let bar_start = fraction_from_ticks(sigmap.bar2tick(i64::from(bar_index), 0));
        let timesig = sigmap.timesig(sigmap.bar2tick(i64::from(bar_index), 0));
        let bar_len = timesig.measure_len();
        if !bar_len.is_positive() {
            continue;
        }
        candidates.extend(detect::detect_tuplets(
            track,
            &keys,
            bar_index,
            bar_start,
            bar_len,
            timesig.numerator,
            basic_quant,
            detect_options,
        ));
    }
    filter::filter_tuplets(
        track,
        &mut candidates,
        basic_quant,
        options.max_voice_count.limit(),
    );
    candidates
}

/// Détection et insertion **précoces** des n-olets, à l'endroit où MuseScore
/// appelle `MidiTuplet::findAllTuplets` (`importmidi.cpp:265`, puis
/// `findTuplets`, `importmidi_tuplet.cpp:1076-1141`).
///
/// contrairement à [`apply_tuplets`] (insertion tardive), les candidats sont
/// filtrés par l'attribution des voix : `assignVoices` retire ceux qui ne
/// trouvent pas de voix libre et rend leurs accords au flux ordinaire. C'est
/// ce qui évite les n-olets inventés dans `v4/timesig-09`.
pub fn detect_and_assign_early(
    track: &mut MTrack,
    sigmap: &TimeSigMap,
    basic_quant: Fraction,
    options: &super::operations::ImportOptions,
    is_human: bool,
) {
    if track.chords.is_empty() || track.drum_track {
        return;
    }
    let mut candidates = detect_all_tuplets(track, sigmap, basic_quant, options, is_human);
    if candidates.is_empty() {
        return;
    }
    let mut by_bar: std::collections::BTreeMap<i32, Vec<TupletInfo>> =
        std::collections::BTreeMap::new();
    for tuplet in candidates.drain(..) {
        let bar_index = super::simplify::find_bar_start(sigmap, tuplet.on_time);
        let index = sigmap
            .tick_values(bar_index.ticks(super::midifile::UNITS_PER_WHOLE as u32))
            .0;
        by_bar.entry(index as i32).or_default().push(tuplet);
    }
    let mut next_seq = track
        .tuplets
        .keys()
        .map(|key| key.1)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for (bar_index, mut tuplets) in by_bar {
        let bar_start = fraction_from_ticks(sigmap.bar2tick(i64::from(bar_index), 0));
        let bar_end = fraction_from_ticks(sigmap.bar2tick(i64::from(bar_index) + 1, 0));
        let bar_len = bar_end - bar_start;
        let mut non_tuplets: Vec<ChordKey> = track
            .chords
            .keys()
            .into_iter()
            .filter(|key| {
                key.0 >= bar_start
                    && key.0 < bar_end
                    && !tuplets.iter().any(|tuplet| tuplet.chords.contains(key))
            })
            .collect();
        // `addChordsBetweenTupletNotes` (`importmidi_tuplet.cpp:1113`) : un
        // accord **hors n-olet** dont l'intervalle recoupe le n-olet rejoint
        // celui-ci si son erreur sur sa grille est plus faible. À faire avant
        // l'attribution des voix, qui utilise les intervalles des accords.
        add_chords_between_early(
            track,
            &mut tuplets,
            &mut non_tuplets,
            bar_start,
            basic_quant,
        );
        // `findTuplets` (`importmidi_tuplet.cpp:1116-1119`) : notes triées par
        // hauteur, n-olets triés par hauteur moyenne, puis partage des
        // premiers accords — seulement si plus d'une voix de n-olet est
        // permise (`tupletVoiceLimit() > 1`, `importmidi_tuplet_voice.cpp:35-49`).
        let allowed = options.max_voice_count.limit();
        let tuplet_voice_limit = if allowed == 1 { 1 } else { allowed - 1 };
        if tuplet_voice_limit > 1 {
            split::sort_notes_by_pitch(track, bar_start, bar_end);
            split::sort_tuplets_by_average_pitch(track, &mut tuplets);
            split::split_first_tuplet_chords(track, &mut tuplets);
            // `findTuplets` : les notes trop longues du premier accord sortent
            // du n-olet avant l'attribution des voix
            // (`importmidi_tuplet.cpp:1118-1119`).
            off_time::minimize_off_time_error(
                track,
                &mut tuplets,
                &mut non_tuplets,
                bar_start,
                basic_quant,
            );
        }
        let prev_bar_start = (bar_start - bar_len).max(Fraction::ZERO);
        let mut back_tied = voice::find_back_tied_tuplets(
            track,
            &tuplets,
            prev_bar_start,
            bar_start,
            basic_quant,
            bar_index,
        );
        voice::assign_voices(
            track,
            &mut tuplets,
            &mut non_tuplets,
            &mut back_tied,
            bar_start,
            basic_quant,
            bar_index,
            options.max_voice_count.limit(),
        );
        voice::add_tuplet_events(track, &tuplets, &back_tied, &mut next_seq);
    }
}

/// Ajoute les accords intermédiaires d'un n-olet (`addChordsBetweenTupletNotes`,
/// `importmidi_tuplet.cpp:533-575`) : un accord non rattaché, dont l'intervalle
/// recoupe le n-olet, rejoint celui-ci si son erreur de n-olet est inférieure à
/// son erreur de quantification régulière.
fn add_chords_between_tuplet_notes(
    track: &MTrack,
    info: &TupletInfo,
    bar_start: Fraction,
    basic_quant: Fraction,
) -> Vec<ChordKey> {
    let mut chords = info.chords.clone();
    let tuplet_data = TupletData {
        voice: 0,
        on_time: bar_start,
        len: info.len,
        tuplet_number: info.tuplet_number,
        element_count: 0,
    };
    let ratio = tuplet_limits(info.tuplet_number).ratio;
    let end = info.on_time + info.len;
    for (chord_key, chord) in track.chords.range(info.on_time, end) {
        if chord_key.0 <= info.on_time || chord.is_in_tuplet || chords.contains(&chord_key) {
            continue;
        }
        let on_time = chord_key.0;
        let off_time = MTrack::max_off_time(chord);
        if off_time < info.on_time || on_time > end {
            continue;
        }
        let tuplet_error = (on_time
            - super::quant::find_quantized_tuplet_chord_on_time(on_time, &tuplet_data, ratio))
        .abs();
        let regular_error = super::quant::find_on_time_quant_error(on_time, chord, basic_quant);
        let mut tuplet_error = tuplet_error;
        let mut regular_error = regular_error;
        if off_time < end {
            tuplet_error += (off_time
                - super::quant::find_quantized_tuplet_note_off_time(
                    on_time,
                    off_time,
                    &tuplet_data,
                    ratio,
                )
                .0)
                .abs();
            regular_error += (off_time
                - super::quant::find_quantized_note_off_time(on_time, off_time, basic_quant).0)
                .abs();
        }
        if tuplet_error < regular_error {
            chords.push(chord_key);
        }
    }
    chords.sort();
    chords
}

/// Version de `addChordsBetweenTupletNotes` utilisée à l'étape précoce :
/// seuls les accords **hors n-olet** peuvent rejoindre un n-olet, et ils sont
/// retirés de la liste après adoption (`importmidi_tuplet.cpp:533-575`).
/// Cela empêche un n-olet de capturer les accords d'un autre n-olet, ce qui
/// faisait perdre six notes sur `tuplet_2_voices_3_5_tuplets`.
fn add_chords_between_early(
    track: &MTrack,
    tuplets: &mut [TupletInfo],
    non_tuplets: &mut Vec<ChordKey>,
    bar_start: Fraction,
    basic_quant: Fraction,
) {
    for tuplet in tuplets {
        let ratio = tuplet_limits(tuplet.tuplet_number).ratio;
        let mut index = 0;
        while index < non_tuplets.len() {
            let key = non_tuplets[index];
            let on_time = key.0;
            let end = tuplet.on_time + tuplet.len;
            let Some(chord) = track.chords.get(key) else {
                index += 1;
                continue;
            };
            let interval = (
                crate::quant::find_min_quantized_on_time(chord, on_time, basic_quant),
                MTrack::max_off_time(chord),
            );
            if on_time > tuplet.on_time && interval.1 > tuplet.on_time && end > interval.0 {
                let mut tuplet_error = (on_time
                    - super::quant::find_quantized_tuplet_chord_on_time(
                        on_time,
                        &TupletData {
                            voice: 0,
                            on_time: bar_start,
                            len: tuplet.len,
                            tuplet_number: tuplet.tuplet_number,
                            element_count: 0,
                        },
                        ratio,
                    ))
                .abs();
                let mut regular_error =
                    super::quant::find_on_time_quant_error(on_time, chord, basic_quant);
                let off_time = MTrack::max_off_time(chord);
                if off_time < end {
                    tuplet_error += (off_time
                        - super::quant::find_quantized_tuplet_note_off_time(
                            on_time,
                            off_time,
                            &TupletData {
                                voice: 0,
                                on_time: bar_start,
                                len: tuplet.len,
                                tuplet_number: tuplet.tuplet_number,
                                element_count: 0,
                            },
                            ratio,
                        )
                        .0)
                        .abs();
                    regular_error += (off_time
                        - super::quant::find_quantized_note_off_time(
                            on_time,
                            off_time,
                            basic_quant,
                        )
                        .0)
                        .abs();
                }
                if tuplet_error < regular_error {
                    tuplet.chords.push(key);
                    tuplet.chords.sort();
                    non_tuplets.remove(index);
                    continue;
                }
            }
            index += 1;
        }
    }
}

/// Insère les n-olets détectés dans la piste (`addTupletEvents`).
pub fn apply_tuplets(
    track: &mut MTrack,
    sigmap: &TimeSigMap,
    candidates: &[TupletInfo],
    basic_quant: Fraction,
) {
    let mut next_seq = track
        .tuplets
        .keys()
        .map(|key| key.1)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for info in candidates {
        let key = (info.on_time, next_seq);
        next_seq = next_seq.saturating_add(1);
        let voice = info
            .chords
            .first()
            .and_then(|key| track.chords.get(*key))
            .map_or(0, |chord| chord.voice);
        let bar_start = super::simplify::find_bar_start(sigmap, info.on_time);
        let chords = add_chords_between_tuplet_notes(track, info, bar_start, basic_quant);
        track.tuplets.insert(
            key,
            TupletData {
                voice,
                on_time: info.on_time,
                len: info.len,
                tuplet_number: info.tuplet_number,
                element_count: 0,
            },
        );
        let mut count = 0;
        for chord_key in &chords {
            if let Some(chord) = track.chords.get_mut(*chord_key) {
                chord.is_in_tuplet = true;
                chord.tuplet = Some(key);
                count += 1;
            }
        }
        if let Some(tuplet) = track.tuplets.get_mut(&key) {
            tuplet.element_count = count;
        }
    }
    remove_empty_tuplets(track);
}

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
        let timesig = sigmap.timesig(sigmap.bar2tick(i64::from(bar_index), 0));
        let bar_len = timesig.measure_len();
        let mut spans = divisions_of_bar_for_tuplets_raw(bar_len, timesig.numerator);
        spans.retain(|span| !span.is_zero());
        spans.sort_by(|left, right| right.cmp(left));

        for span in spans {
            let beat = super::meter::beat_length_raw(bar_len, timesig.numerator);
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

        // `isTupletAllowed` (`importmidi_tuplet_detect.cpp:75-80`) n'exige
        // qu'une note atteignant la moitié d'une case ; `detectStaccato`
        // (`:265-278`) écarte ensuite les n-olets dont la majorité des notes
        // sont piquées. Une seule note courte est tolérée : cela accepte le
        // triolet lié de `tuplet_tied_3_5_tuplets.mid` sans confondre les
        // notes staccato de `tuplet_7_staccato.mid`.
        let short_count = matched
            .iter()
            .filter(|index| {
                let (on_time, _, off_time, _) = chords[**index];
                off_time - on_time < grid * Fraction::HALF
            })
            .count();
        let main_line = short_count <= 1;
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

/// Vrai si deux intervalles se recoupent (`haveIntersection`,
/// `importmidi_inner.cpp:222-230`).
fn have_intersection(
    first: (Fraction, Fraction),
    second: (Fraction, Fraction),
    strict: bool,
) -> bool {
    if strict {
        first.1 > second.0 && first.0 < second.1
    } else {
        first.1 >= second.0 && first.0 <= second.1
    }
}

/// N-olets d'une voix qui recoupent un intervalle
/// (`MidiTuplet::findTupletsForTimeRange`).
pub fn find_tuplets_for_time_range(
    track: &MTrack,
    voice: usize,
    on_time: Fraction,
    len: Fraction,
    strict: bool,
) -> Vec<TupletKey> {
    let interval = (on_time, on_time + len);
    let mut result = Vec::new();
    for (key, tuplet) in &track.tuplets {
        if tuplet.voice != voice {
            continue;
        }
        let tuplet_interval = (tuplet.on_time, tuplet.on_time + tuplet.len);
        if have_intersection(interval, tuplet_interval, strict) {
            result.push(*key);
        }
    }
    result
}

/// Vrai si l'accord a une note qui ne remplit pas le n-olet
/// (`MidiTuplet::hasNonTrivialChord`, `importmidi_tuplet.cpp:77-105`).
pub fn has_non_trivial_chord(
    chord_on_time: Fraction,
    notes: &[super::chord::MidiNote],
    tuplet_on_time: Fraction,
    tuplet_len: Fraction,
) -> bool {
    if chord_on_time == tuplet_on_time {
        notes
            .iter()
            .any(|note| note.off_time - chord_on_time < tuplet_len)
    } else {
        if chord_on_time > tuplet_on_time && chord_on_time < tuplet_on_time + tuplet_len {
            return true;
        }
        if chord_on_time >= tuplet_on_time + tuplet_len {
            return false;
        }
        notes
            .iter()
            .any(|note| note.off_time < tuplet_on_time + tuplet_len)
    }
}

/// Vrai si le n-olet ne contient plus aucun accord qui en a besoin
/// (`MidiTuplet::isTupletUseless`, `importmidi_tuplet.cpp:109-130`).
pub fn is_tuplet_useless(
    track: &MTrack,
    voice: usize,
    on_time: Fraction,
    len: Fraction,
    max_chord_length: Fraction,
) -> bool {
    let found = super::chord::find_chords_for_time_range(
        track,
        voice,
        on_time,
        on_time + len,
        max_chord_length,
    );
    for key in found {
        if let Some(chord) = track.chords.get(key) {
            if has_non_trivial_chord(key.0, &chord.notes, on_time, len) {
                return false;
            }
        }
    }
    true
}

/// Retire un n-olet et les références qu'y font les accords et les notes
/// (`MidiTuplet::removeTuplet`, `importmidi_tuplet.cpp:132-161`).
pub fn remove_tuplet(track: &mut MTrack, key: TupletKey, max_chord_length: Fraction) {
    let Some(tuplet) = track.tuplets.remove(&key) else {
        return;
    };
    let end = tuplet.on_time + tuplet.len;
    let keys = track.chords.keys();
    for chord_key in keys {
        if chord_key.0 + max_chord_length <= tuplet.on_time {
            continue;
        }
        if chord_key.0 >= end {
            continue;
        }
        if let Some(chord) = track.chords.get_mut(chord_key) {
            if chord.tuplet == Some(key) {
                chord.is_in_tuplet = false;
                chord.tuplet = None;
            }
            for note in &mut chord.notes {
                if note.tuplet == Some(key) {
                    note.is_in_tuplet = false;
                    note.tuplet = None;
                }
            }
        }
    }
}
