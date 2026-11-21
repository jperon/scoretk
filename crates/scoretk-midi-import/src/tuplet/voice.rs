// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_tuplet_voice.cpp` et de la fonction `findTuplets` de
// `importmidi_tuplet.cpp` de MuseScore Studio (Copyright (C) MuseScore Limited
// and others), modifié (réécrit en Rust) en 2026. Référence :
// `src/importexport/midi/internal/midiimport/importmidi_tuplet_voice.cpp`.
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

//! Attribution des voix aux n-olets **pendant la détection**
//! (`MidiTuplet::assignVoices`, `importmidi_tuplet_voice.cpp:872-920`).
//!
//! C'est cette étape, et elle seule, qui permet d'éliminer les n-olets
//! inventés : un candidat qui ne trouve pas de voix libre est retiré
//! (`removeUnusedTuplets`) et ses accords redeviennent des accords ordinaires.
//! Les n-olets liés en arrière (`findBackTiedTuplets`) gardent la voix de la
//! mesure précédente quand elle est encore disponible.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::Fraction;

use super::{TupletInfo, TupletKey};
use crate::chord::{chord_average_pitch, min_allowed_duration, ChordKey, MTrack};
use crate::quant::{
    find_max_quantized_off_time, find_max_quantized_tuplet_off_time, find_min_quantized_on_time,
    find_quantized_note_off_time, find_quantized_tuplet_chord_on_time,
    find_quantized_tuplet_note_off_time, quant_for_len,
};

/// Paire `(début, fin)` d'un intervalle occupé dans une voix.
type Interval = (Fraction, Fraction);
/// Intervalles par voix (`std::map<int, std::vector<Interval>>`).
type VoiceIntervals = BTreeMap<usize, Vec<Interval>>;

/// `TiedTuplet` (`importmidi_tuplet_voice.h`) : accord antérieur lié à un
/// n-olet. `voice == None` correspond au `-1` du C++ (voix non encore choisie).
#[derive(Clone, Debug)]
pub struct TiedTuplet {
    pub tuplet_id: usize,
    pub voice: Option<usize>,
    pub chord: ChordKey,
    pub tied_note_indexes: Vec<usize>,
}

/// `MidiTuplet::tupletVoiceLimit` (`:35-47`) : une voix est réservée aux
/// accords hors n-olet dès que plusieurs voix sont autorisées.
pub fn tuplet_voice_limit(limit: usize) -> usize {
    if limit <= 1 {
        1
    } else {
        limit - 1
    }
}

/// `haveIntersection` : les bornes sont semi-ouvertes.
fn have_intersection(first: Interval, second: Interval) -> bool {
    first.1 > second.0 && second.1 > first.0
}

/// `tupletInterval` (`importmidi_tuplet.cpp:252-267`).
fn tuplet_interval(track: &MTrack, tuplet: &TupletInfo, basic_quant: Fraction) -> Interval {
    let mut end = tuplet.on_time + tuplet.len;
    for key in &tuplet.chords {
        if let Some(chord) = track.chords.get(*key) {
            let off = find_max_quantized_off_time(chord, key.0, basic_quant);
            if off > end {
                end = off;
            }
        }
    }
    (tuplet.on_time, end)
}

/// `chordInterval` (`importmidi_tuplet_voice.cpp:49-67`).
///
/// `findOnTimeBetweenChords` (`:957-1041`) n'est pas porté : l'attaque de
/// l'accord est utilisée, bornée par le début de mesure, comme dans le reste
/// du port. Les fins sont quantifiées, comme dans le C++.
fn chord_interval(
    track: &MTrack,
    key: ChordKey,
    basic_quant: Fraction,
    bar_start: Fraction,
) -> Interval {
    let Some(chord) = track.chords.get(key) else {
        return (bar_start, bar_start + basic_quant);
    };
    let mut on_time = key.0.max(bar_start);
    let mut off_time = find_max_quantized_off_time(chord, key.0, basic_quant);
    if off_time < on_time {
        on_time = find_min_quantized_on_time(chord, key.0, basic_quant).max(bar_start);
        off_time = on_time;
    }
    if off_time == on_time {
        off_time += quant_for_len(min_allowed_duration(), basic_quant);
    }
    (on_time, off_time)
}

/// `setTupletVoice` (`:101-110`).
fn set_tuplet_voice(track: &mut MTrack, tuplet: &TupletInfo, voice: usize) {
    for key in &tuplet.chords {
        if let Some(chord) = track.chords.get_mut(*key) {
            chord.voice = voice;
        }
    }
}

/// `setTupletVoices` (`:112-134`) : premier n-olet pendant, première voix
/// libre, dans l'ordre des voix.
fn set_tuplet_voices(
    track: &mut MTrack,
    tuplets: &[TupletInfo],
    pending: &mut BTreeSet<usize>,
    intervals: &mut VoiceIntervals,
    _basic_quant: Fraction,
    limit: usize,
) {
    let mut voice = 0;
    while !pending.is_empty() && voice < limit {
        let ids: Vec<usize> = pending.iter().copied().collect();
        for id in ids {
            let Some(tuplet) = tuplets.iter().find(|tuplet| tuplet.id == id) else {
                continue;
            };
            // Intervalle nominal : l'extension jusqu'à la fin quantifiée du
            // dernier accord (`tupletInterval`) peut faire chevaucher deux
            // n-olets successifs à cause d'une note liée en arrière, alors
            // que MuseScore 4.7.4 les place dans la même voix.
            let interval = (tuplet.on_time, tuplet.on_time + tuplet.len);
            if intervals
                .get(&voice)
                .is_none_or(|used| !used.iter().any(|u| have_intersection(interval, *u)))
            {
                set_tuplet_voice(track, tuplet, voice);
                intervals.entry(voice).or_default().push(interval);
                pending.remove(&id);
            }
        }
        voice += 1;
    }
}

/// `findPitchDist` (`:136-168`) : distance de hauteur avec le premier n-olet
/// de cette voix (`i32::MAX` s'il n'y en a pas).
fn find_pitch_dist(track: &MTrack, tuplets: &[TupletInfo], voice: usize, chord_pitch: i32) -> i32 {
    for tuplet in tuplets {
        let first = tuplet.chords.first().and_then(|key| track.chords.get(*key));
        if first.is_none_or(|chord| chord.voice != voice) {
            continue;
        }
        let mut sum = 0;
        let mut count = 0;
        for key in &tuplet.chords {
            if let Some(chord) = track.chords.get(*key) {
                sum += chord_average_pitch(chord);
                count += 1;
            }
        }
        if count > 0 && sum != 0 {
            return (chord_pitch - (f64::from(sum) / f64::from(count)).round() as i32).abs();
        }
    }
    i32::MAX
}

/// `findTiedNotes` (`:745-799`) : notes d'un accord antérieur dont la fin
/// quantifiée tombe mieux sur la grille du n-olet que sur la grille ordinaire.
fn find_tied_notes(
    track: &MTrack,
    tuplet: &TupletInfo,
    key: ChordKey,
    bar_start: Fraction,
    basic_quant: Fraction,
) -> Vec<usize> {
    let ratio = super::tuplet_limits(tuplet.tuplet_number).ratio;
    let data = crate::tuplet::TupletData {
        voice: 0,
        on_time: bar_start,
        len: tuplet.len,
        tuplet_number: tuplet.tuplet_number,
        element_count: 0,
    };
    let first_on_time = tuplet.chords.first().map_or(tuplet.on_time, |key| {
        find_quantized_tuplet_chord_on_time(key.0, &data, ratio)
    });
    let Some(chord) = track.chords.get(key) else {
        return Vec::new();
    };
    if find_max_quantized_tuplet_off_time(chord, key.0, tuplet.len, ratio, bar_start)
        > first_on_time
    {
        return Vec::new();
    }
    if key.0.max(bar_start) >= tuplet.on_time {
        return Vec::new();
    }
    let mut tied = Vec::new();
    for (index, note) in chord.notes.iter().enumerate() {
        let (off_in_tuplet, _) =
            find_quantized_tuplet_note_off_time(key.0, note.off_time, &data, ratio);
        if off_in_tuplet < bar_start || off_in_tuplet <= tuplet.on_time {
            continue;
        }
        let (regular_off, _) = find_quantized_note_off_time(key.0, note.off_time, basic_quant);
        if (note.off_time - off_in_tuplet).abs() > (note.off_time - regular_off).abs() {
            continue;
        }
        tied.push(index);
    }
    tied
}

/// `areTupletsIntersect` (`:734-743`). Le C++ 5.0 calcule `endTime2` depuis
/// `onTime1` (manifestement une faute de frappe) ; le comportement de
/// référence 4.7.4 est restauré ici en utilisant la fin réelle du second
/// n-olet, sinon deux n-olets successifs sont toujours considérés comme
/// intersectants et le n-olet lié en arrière n'est jamais trouvé.
fn are_tuplets_intersect(first: &TupletInfo, second: &TupletInfo) -> bool {
    let end_first = first.on_time + first.len;
    let end_second = second.on_time + second.len;
    end_first > second.on_time && first.on_time < end_second
}

/// `findBackTiedTuplets` (`:801-870`).
pub fn find_back_tied_tuplets(
    track: &MTrack,
    tuplets: &[TupletInfo],
    prev_bar_start: Fraction,
    bar_start: Fraction,
    basic_quant: Fraction,
    current_bar_index: i32,
) -> Vec<TiedTuplet> {
    let mut tied = Vec::new();
    let mut back_intervals: BTreeMap<i32, Vec<Interval>> = BTreeMap::new();
    let mut used_chords = BTreeSet::new();
    let mut used_tuplets = BTreeSet::new();
    let mut tuplet_chords: BTreeMap<ChordKey, usize> = BTreeMap::new();
    for (index, tuplet) in tuplets.iter().enumerate() {
        for key in &tuplet.chords {
            tuplet_chords.insert(*key, index);
        }
    }
    let keys = track.chords.keys();

    for tuplet in tuplets {
        let Some(first_key) = tuplet.chords.first() else {
            continue;
        };
        let Some(first_pos) = keys.iter().position(|key| key == first_key) else {
            continue;
        };
        let mut cursor = first_pos;
        while cursor > 0 {
            cursor -= 1;
            let key = keys[cursor];
            if key.0 < prev_bar_start {
                break;
            }
            if tuplet_chords.get(&key).is_some_and(|index| {
                tuplets
                    .get(*index)
                    .is_some_and(|other| are_tuplets_intersect(tuplet, other))
            }) {
                continue;
            }
            let chord = track.chords.get(key).expect("clé de la table d'accords");
            let voice = if chord.bar_index < current_bar_index {
                Some(chord.voice)
            } else {
                None
            };
            let interval = (tuplet.on_time, tuplet.on_time + tuplet.len);
            let voice_key = voice.map_or(-1, |voice| voice as i32);
            if voice.is_some()
                && back_intervals
                    .get(&voice_key)
                    .is_some_and(|used| used.iter().any(|u| have_intersection(interval, *u)))
            {
                continue;
            }
            let notes = find_tied_notes(track, tuplet, key, bar_start, basic_quant);
            if notes.is_empty() || used_chords.contains(&key) {
                continue;
            }
            let tuplet_index = tuplet_chords.get(&key).copied().unwrap_or(usize::MAX);
            if used_tuplets.contains(&tuplet_index) {
                continue;
            }
            tied.push(TiedTuplet {
                tuplet_id: tuplet.id,
                voice,
                chord: key,
                tied_note_indexes: notes,
            });
            back_intervals.entry(voice_key).or_default().push(interval);
            used_chords.insert(key);
            used_tuplets.insert(tuplet_index);
            break;
        }
    }
    tied
}

/// Donne une voix aux n-olets liés en arrière (`setBackTiedVoices`,
/// `:652-718`). Une liaison sans voix disponible est abandonnée : le n-olet
/// reste à placer par `setTupletVoices`.
#[allow(clippy::too_many_arguments)]
fn set_back_tied_voices(
    track: &mut MTrack,
    back_tied: &mut Vec<TiedTuplet>,
    tuplets: &[TupletInfo],
    pending: &mut BTreeSet<usize>,
    intervals: &mut VoiceIntervals,
    bar_start: Fraction,
    basic_quant: Fraction,
    limit: usize,
) {
    let mut used: BTreeMap<usize, Vec<Interval>> = BTreeMap::new();
    for tied in back_tied.iter() {
        let Some(tuplet) = tuplets.iter().find(|t| t.id == tied.tuplet_id) else {
            continue;
        };
        let interval = (tuplet.on_time, tuplet.on_time + tuplet.len);
        if let Some(voice) = tied.voice {
            used.entry(voice).or_default().push(interval);
        }
    }

    let mut index = 0;
    while index < back_tied.len() {
        let Some(tuplet) = tuplets
            .iter()
            .find(|t| t.id == back_tied[index].tuplet_id)
            .cloned()
        else {
            index += 1;
            continue;
        };
        if !pending.contains(&tuplet.id) {
            index += 1;
            continue;
        }
        let interval = tuplet_interval(track, &tuplet, basic_quant);
        let voice = match back_tied[index].voice {
            Some(voice) if voice < limit => Some(voice),
            Some(_) => None,
            None => (0..limit).find(|voice| {
                !used
                    .get(voice)
                    .is_some_and(|list| list.iter().any(|used| have_intersection(interval, *used)))
            }),
        };
        let Some(voice) = voice else {
            back_tied.remove(index);
            continue;
        };
        back_tied[index].voice = Some(voice);
        set_tuplet_voice(track, &tuplet, voice);
        intervals.entry(voice).or_default().push(chord_interval(
            track,
            back_tied[index].chord,
            basic_quant,
            bar_start,
        ));
        intervals.entry(voice).or_default().push(interval);
        used.entry(voice).or_default().push(interval);
        pending.remove(&tuplet.id);
        index += 1;
    }
}

/// `eraseBackTiedTuplet(int, …)` (`:328-340`).
fn erase_back_tied(tuplets: &mut Vec<TiedTuplet>, id: usize) {
    if let Some(index) = tuplets.iter().position(|tied| tied.tuplet_id == id) {
        tuplets.remove(index);
    }
}

/// `removeUnusedTuplets` (`:413-441`).
fn remove_unused_tuplets(
    track: &mut MTrack,
    tuplets: &mut Vec<TupletInfo>,
    non_tuplets: &mut Vec<ChordKey>,
    pending: &mut BTreeSet<usize>,
    back_tied: &mut Vec<TiedTuplet>,
    pending_non_tuplets: &mut BTreeSet<ChordKey>,
    bar_index: i32,
) {
    if pending.is_empty() {
        return;
    }
    let mut kept = Vec::new();
    for tuplet in tuplets.drain(..) {
        if !pending.contains(&tuplet.id) {
            kept.push(tuplet);
            continue;
        }
        erase_back_tied(back_tied, tuplet.id);
        for key in &tuplet.chords {
            let belongs = track
                .chords
                .get(*key)
                .is_some_and(|chord| chord.bar_index == bar_index);
            if belongs {
                non_tuplets.push(*key);
                pending_non_tuplets.insert(*key);
            }
        }
    }
    *tuplets = kept;
    pending.clear();
}

/// `excludeExtraVoiceTuplets` (`:342-411`), utilisé quand une seule voix est
/// autorisée : un n-olet qui recoupe un accord ordinaire est retiré.
fn exclude_extra_voice_tuplets(
    track: &MTrack,
    tuplets: &mut Vec<TupletInfo>,
    non_tuplets: &mut Vec<ChordKey>,
    back_tied: &mut Vec<TiedTuplet>,
    bar_start: Fraction,
    basic_quant: Fraction,
    bar_index: i32,
) -> bool {
    if tuplets.is_empty() {
        return false;
    }
    let mut added: Vec<ChordKey> = non_tuplets.clone();
    let mut excluded = false;
    loop {
        let mut new_non_tuplets = Vec::new();
        let count = added.len();
        let mut index = 0;
        while index < tuplets.len() {
            let interval = tuplet_interval(track, &tuplets[index], basic_quant);
            let overlaps = added.iter().take(count).any(|key| {
                have_intersection(
                    interval,
                    chord_interval(track, *key, basic_quant, bar_start),
                )
            });
            let tied = added.iter().take(count).any(|key| {
                back_tied
                    .iter()
                    .any(|tied| tied.tuplet_id == tuplets[index].id && tied.chord == *key)
            });
            if overlaps && !tied {
                for key in &tuplets[index].chords {
                    if track
                        .chords
                        .get(*key)
                        .is_some_and(|chord| chord.bar_index == bar_index)
                    {
                        new_non_tuplets.push(*key);
                    }
                }
                erase_back_tied(back_tied, tuplets[index].id);
                tuplets.remove(index);
                excluded = true;
            } else {
                index += 1;
            }
        }
        if new_non_tuplets.is_empty() {
            break;
        }
        non_tuplets.extend(new_non_tuplets.iter().copied());
        added = new_non_tuplets;
    }
    excluded
}

/// `setNonTupletVoices` (`:171-206`), avec les intervalles des n-olets déjà
/// attribués. La voix retenue est celle qui n'intersecte pas ces intervalles
/// et dont la hauteur moyenne est la plus proche.
fn set_non_tuplet_voices(
    track: &mut MTrack,
    pending: &BTreeSet<ChordKey>,
    intervals: &VoiceIntervals,
    tuplets: &[TupletInfo],
    bar_start: Fraction,
    basic_quant: Fraction,
    limit: usize,
) {
    for key in pending {
        let Some(chord) = track.chords.get(*key) else {
            continue;
        };
        let interval = chord_interval(track, *key, basic_quant, bar_start);
        let pitch = chord_average_pitch(chord);
        let mut best = None;
        for voice in 0..limit {
            if intervals
                .get(&voice)
                .is_some_and(|used| used.iter().any(|u| have_intersection(interval, *u)))
            {
                continue;
            }
            let dist = find_pitch_dist(track, tuplets, voice, pitch);
            if best.is_none_or(|(_, min): (usize, i32)| dist < min) {
                best = Some((voice, dist));
            }
        }
        if let Some(voice) = best.map(|(voice, _)| voice) {
            if let Some(chord) = track.chords.get_mut(*key) {
                chord.voice = voice;
            }
        }
    }
}

/// `MidiTuplet::assignVoices` (`:872-920`).
#[allow(clippy::too_many_arguments)]
pub fn assign_voices(
    track: &mut MTrack,
    tuplets: &mut Vec<TupletInfo>,
    non_tuplets: &mut Vec<ChordKey>,
    back_tied: &mut Vec<TiedTuplet>,
    bar_start: Fraction,
    basic_quant: Fraction,
    bar_index: i32,
    voice_limit: usize,
) {
    let mut pending: BTreeSet<usize> = tuplets.iter().map(|tuplet| tuplet.id).collect();
    let mut pending_non: BTreeSet<ChordKey> = non_tuplets.iter().copied().collect();
    let mut intervals: VoiceIntervals = BTreeMap::new();
    let tuplet_limit = tuplet_voice_limit(voice_limit);

    set_back_tied_voices(
        track,
        back_tied,
        tuplets,
        &mut pending,
        &mut intervals,
        bar_start,
        basic_quant,
        tuplet_limit,
    );
    set_tuplet_voices(
        track,
        tuplets,
        &mut pending,
        &mut intervals,
        basic_quant,
        tuplet_limit,
    );
    remove_unused_tuplets(
        track,
        tuplets,
        non_tuplets,
        &mut pending,
        back_tied,
        &mut pending_non,
        bar_index,
    );
    if tuplet_limit == 1
        && exclude_extra_voice_tuplets(
            track,
            tuplets,
            non_tuplets,
            back_tied,
            bar_start,
            basic_quant,
            bar_index,
        )
    {
        intervals.clear();
        for tuplet in tuplets.iter() {
            let voice = tuplet
                .chords
                .first()
                .and_then(|key| track.chords.get(*key))
                .map_or(0, |chord| chord.voice);
            intervals
                .entry(voice)
                .or_default()
                .push(tuplet_interval(track, tuplet, basic_quant));
        }
    }
    set_non_tuplet_voices(
        track,
        &pending_non,
        &intervals,
        tuplets,
        bar_start,
        basic_quant,
        voice_limit,
    );
}

/// `addTupletEvents` (`importmidi_tuplet.cpp:862-922`) : insère les n-olets
/// retenus dans la piste et marque leurs accords et leurs notes.
pub fn add_tuplet_events(
    track: &mut MTrack,
    tuplets: &[TupletInfo],
    back_tied: &[TiedTuplet],
    next_seq: &mut u64,
) -> Vec<TupletKey> {
    let mut inserted = Vec::new();
    for tuplet in tuplets {
        let voice = tuplet
            .chords
            .first()
            .and_then(|key| track.chords.get(*key))
            .map_or(0, |chord| chord.voice);
        let key: TupletKey = (tuplet.on_time, *next_seq);
        *next_seq += 1;
        track.tuplets.insert(
            key,
            super::TupletData {
                voice,
                on_time: tuplet.on_time,
                len: tuplet.len,
                tuplet_number: tuplet.tuplet_number,
                element_count: tuplet.chords.len(),
            },
        );
        inserted.push(key);
        for chord_key in &tuplet.chords {
            if let Some(chord) = track.chords.get_mut(*chord_key) {
                chord.is_in_tuplet = true;
                chord.tuplet = Some(key);
                for note in &mut chord.notes {
                    if note.off_time <= tuplet.on_time + tuplet.len {
                        note.is_in_tuplet = true;
                        note.tuplet = Some(key);
                    }
                }
            }
        }
        for tied in back_tied.iter().filter(|tied| tied.tuplet_id == tuplet.id) {
            if let Some(chord) = track.chords.get_mut(tied.chord) {
                for index in &tied.tied_note_indexes {
                    if let Some(note) = chord.notes.get_mut(*index) {
                        note.is_in_tuplet = true;
                        note.tuplet = Some(key);
                    }
                }
            }
        }
    }
    inserted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::{MidiChord, MidiNote};

    fn chord(on_time: Fraction, pitch: i32) -> MidiChord {
        MidiChord {
            notes: vec![MidiNote::new(
                pitch,
                80,
                on_time + Fraction::new(1, 8),
                on_time,
            )],
            ..MidiChord::default()
        }
    }

    /// `removeUnusedTuplets` : avec une seule voix disponible, le second
    /// n-olet qui chevauche le premier ne trouve pas de voix libre ; il est
    /// retiré et ses accords redeviennent des accords ordinaires.
    #[test]
    fn n_olet_sans_voix_libre_est_retire() {
        let mut track = MTrack::default();
        let first = track
            .chords
            .insert(Fraction::ZERO, chord(Fraction::ZERO, 60));
        let second = track
            .chords
            .insert(Fraction::new(1, 8), chord(Fraction::new(1, 8), 62));
        let third = track
            .chords
            .insert(Fraction::new(1, 4), chord(Fraction::new(1, 4), 64));
        let mut tuplets = vec![
            TupletInfo {
                id: 1,
                tuplet_number: 3,
                on_time: Fraction::ZERO,
                len: Fraction::new(1, 2),
                chords: vec![first, second],
                ..TupletInfo::default()
            },
            TupletInfo {
                id: 2,
                tuplet_number: 3,
                on_time: Fraction::new(1, 8),
                len: Fraction::new(1, 2),
                chords: vec![second, third],
                ..TupletInfo::default()
            },
        ];
        let mut non_tuplets = Vec::new();
        let mut back_tied = Vec::new();
        assign_voices(
            &mut track,
            &mut tuplets,
            &mut non_tuplets,
            &mut back_tied,
            Fraction::ZERO,
            Fraction::new(1, 16),
            0,
            1,
        );
        assert!(
            tuplets.is_empty(),
            "aucun des deux n-olets ne peut rester avec une seule voix : {tuplets:?}"
        );
        assert!(
            non_tuplets.contains(&second),
            "les accords des n-olets retirés reviennent au flux ordinaire"
        );
    }
}
