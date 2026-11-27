// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_chord.cpp` et `importmidi_chord.h` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Accords MIDI intermédiaires et opérations de `MChord`.
//!
//! Portage de `MidiNote`, `MidiChord` et `MTrack`
//! (`importmidi_chord.h`), et des fonctions de `MChord`
//! (`importmidi_chord.cpp`) : collecte des notes en accords, fusion, découpe
//! des accords à durées inégales, suppression des chevauchements, index de
//! mesure.

use std::collections::BTreeMap;

use scoretk_core::instruments::Template;
use scoretk_core::model::{Fraction, StaffId};

use super::key::KeyList;
use super::midifile::{DIVISION, UNITS_PER_WHOLE};
use super::timesigmap::TimeSigMap;
use super::tuplet::{TupletData, TupletKey};

/// Clé d'un accord dans [`ChordMap`] : date d'attaque et rang d'insertion.
pub type ChordKey = (Fraction, u64);

/// Durée minimale autorisée d'une note (`MChord::minAllowedDuration`).
///
/// Vaut `DIVISION / 32`, soit 1/128 de ronde.
pub fn min_allowed_duration() -> Fraction {
    Fraction::from_ticks(DIVISION / 32, UNITS_PER_WHOLE as u32)
}

/// Note MIDI en cours d'import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiNote {
    /// Hauteur MIDI.
    pub pitch: i32,
    /// Vélocité.
    pub velocity: i32,
    /// Canal MIDI de l'attaque (nibble de statut du Note On).
    pub channel: u8,
    /// Fin de la note.
    pub off_time: Fraction,
    /// Indice de la note liée précédente, si cette note prolonge une liaison.
    pub tie: Option<usize>,
    /// Vrai si la note doit porter un point d'articulation (note piquée
    /// déduite d'une durée courte).
    pub staccato: bool,
    /// Vrai si la note appartient à un n-olet.
    pub is_in_tuplet: bool,
    /// N-olet de la note, si elle en relève.
    pub tuplet: Option<TupletKey>,
    /// Quantification finale de la fin, pour la simplification des durées.
    pub off_time_quant: Option<Fraction>,
    /// Attaque d'origine, pour l'attribution des paroles.
    pub orig_on_time: Fraction,
}

impl MidiNote {
    /// Construit une note MIDI.
    pub fn new(pitch: i32, velocity: i32, off_time: Fraction, orig_on_time: Fraction) -> Self {
        Self {
            pitch,
            velocity,
            channel: 0,
            off_time,
            tie: None,
            staccato: false,
            is_in_tuplet: false,
            tuplet: None,
            off_time_quant: None,
            orig_on_time,
        }
    }
}

/// Accord MIDI en cours d'import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiChord {
    /// Voix (0-3).
    pub voice: usize,
    /// Notes de l'accord.
    pub notes: Vec<MidiNote>,
    /// Vrai si l'accord appartient à un n-olet.
    pub is_in_tuplet: bool,
    /// Index de mesure (`-1` tant qu'il n'est pas fixé).
    pub bar_index: i32,
    /// N-olet de l'accord, si les notes n'ont pas toutes le leur.
    pub tuplet: Option<TupletKey>,
}

impl MidiChord {
    /// Vrai si au moins une note est marquée piquée (`MidiChord::isStaccato`).
    pub fn is_staccato(&self) -> bool {
        self.notes.iter().any(|note| note.staccato)
    }
}

/// Table ordonnée d'accords : équivalent de `std::multimap<ReducedFraction,
/// MidiChord>`.
///
/// La clé porte un rang d'insertion pour autoriser plusieurs accords à la même
/// date, et l'itération suit `(date, rang)` dans l'ordre du C++.
#[derive(Clone, Debug, Default)]
pub struct ChordMap {
    entries: BTreeMap<ChordKey, MidiChord>,
    next_seq: u64,
}

impl ChordMap {
    /// Construit une table vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Nombre d'accords.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Vrai si la table est vide.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insère un accord et renvoie sa clé.
    pub fn insert(&mut self, on_time: Fraction, chord: MidiChord) -> ChordKey {
        let key = (on_time, self.next_seq);
        self.next_seq += 1;
        self.entries.insert(key, chord);
        key
    }

    /// Insère un accord avec une clé explicite (après une fusion).
    pub fn insert_at(&mut self, key: ChordKey, chord: MidiChord) {
        self.next_seq = self.next_seq.max(key.1 + 1);
        self.entries.insert(key, chord);
    }

    /// Accès à un accord.
    pub fn get(&self, key: ChordKey) -> Option<&MidiChord> {
        self.entries.get(&key)
    }

    /// Accès mutable à un accord.
    pub fn get_mut(&mut self, key: ChordKey) -> Option<&mut MidiChord> {
        self.entries.get_mut(&key)
    }

    /// Retire un accord.
    pub fn remove(&mut self, key: ChordKey) -> Option<MidiChord> {
        self.entries.remove(&key)
    }

    /// Itère sur les accords dans l'ordre `(date, rang)`, avec leur date.
    pub fn iter(&self) -> impl Iterator<Item = (Fraction, &MidiChord)> {
        self.entries
            .iter()
            .map(|((on_time, _), chord)| (*on_time, chord))
    }

    /// Itère sur les accords avec leur clé complète et leur date.
    pub fn iter_with_keys(&self) -> impl Iterator<Item = (&ChordKey, &MidiChord)> {
        self.entries.iter()
    }

    /// Itère mutablement sur les accords, avec leur date.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Fraction, &mut MidiChord)> {
        self.entries
            .iter_mut()
            .map(|((on_time, _), chord)| (*on_time, chord))
    }

    /// Clés des accords, dans l'ordre.
    pub fn keys(&self) -> Vec<ChordKey> {
        self.entries.keys().copied().collect()
    }

    /// Accords dont l'attaque est exactement `on_time`.
    pub fn keys_at(&self, on_time: Fraction) -> Vec<ChordKey> {
        self.entries
            .range((on_time, 0)..=(on_time, u64::MAX))
            .map(|(key, _)| *key)
            .collect()
    }

    /// Accords dont l'attaque est dans `[start, end)`.
    ///
    /// Équivalent d'un `lowerBound`/`upperBound` sur la `std::multimap` du C++
    /// (`MidiTuplet::findAllTuplets` ne parcourt que les accords de la mesure),
    /// évite de rescanner toute la piste à chaque position candidate.
    pub fn range(
        &self,
        start: Fraction,
        end: Fraction,
    ) -> impl Iterator<Item = (ChordKey, &MidiChord)> {
        self.entries
            .range((start, 0)..(end, 0))
            .map(|(key, chord)| (*key, chord))
    }

    /// Prochain accord après `key` appartenant à `voice`.
    ///
    /// Évite de rescanner toute la table pour chaque accord
    /// (`minimizeNumberOfRests` est quadratique sans cela).
    pub fn next_in_voice(&self, key: ChordKey, voice: usize) -> Option<(ChordKey, &MidiChord)> {
        // Borne exclusive sur `(date, rang maximal)` : tous les accords à la
        // même date sont écartés, comme le test `*next_on_time > key.0` d'origine.
        self.entries
            .range((
                std::ops::Bound::Excluded((key.0, u64::MAX)),
                std::ops::Bound::Unbounded,
            ))
            .find(|(_, chord)| chord.voice == voice)
            .map(|(key, chord)| (*key, chord))
    }

    /// Premier accord de `voice` dont la date est au moins `from`, en partant
    /// de l'accord `key` exclu.
    ///
    /// Équivalent du `std::next(it)` puis du filtre
    /// `next->second.voice != voice || next->first < note.offTime` de
    /// `minimizeNumberOfRests` (`importmidi_simplify.cpp:257-262`) : un accord
    /// qui commence avant la fin de la note est ignoré, même dans la même voix.
    pub fn next_in_voice_at_or_after(
        &self,
        key: ChordKey,
        voice: usize,
        from: Fraction,
    ) -> Option<(ChordKey, &MidiChord)> {
        self.entries
            .range((from, 0)..)
            .find(|(next_key, chord)| **next_key != key && chord.voice == voice)
            .map(|(next_key, chord)| (*next_key, chord))
    }
}

/// Piste MIDI en cours d'import (`MTrack`).
#[derive(Clone, Debug)]
pub struct MTrack {
    /// Index de la piste dans le fichier MIDI.
    pub index: usize,
    /// Index d'opération (ordre d'import de la piste).
    pub index_of_operation: usize,
    /// Programme GM (0-127) ou 128 pour la percussion.
    pub program: u8,
    /// Vrai pour une piste de percussion.
    pub drum_track: bool,
    /// Canal de sortie du fichier.
    pub out_channel: i32,
    /// Nom de piste (`META_TRACK_NAME`).
    pub name: String,
    /// Volume du canal (`CTRL_VOLUME`), par tick.
    pub volumes: BTreeMap<Fraction, u8>,
    /// Panoramique du canal (`CTRL_PANPOT`), si présent.
    pub pan: Option<u8>,
    /// Évènements bruts de la piste, pour les méta-évènements et les paroles.
    pub events: Vec<(i64, super::midifile::RawEvent)>,
    /// Accords.
    pub chords: ChordMap,
    /// N-olets trouvés.
    pub tuplets: BTreeMap<TupletKey, TupletData>,
    /// Division du fichier.
    pub division: i64,
    /// Vrai si la division est en ticks par seconde.
    pub is_division_in_tps: bool,
    /// Vrai si la piste portait des notes dans le fichier.
    pub had_initial_notes: bool,
    /// Vrai si le fichier portait une armure pour cette piste.
    pub has_key: bool,
    /// Armures lues dans le fichier.
    pub key_list: KeyList,
    /// Portée créée dans le modèle.
    pub staff: Option<StaffId>,
    /// Gabarit MuseScore retenu (`msInstrList[msInstrIndex]`, index 0), rempli
    /// par `findInstrumentsForAllTracks`.
    pub ms_instr: Option<&'static Template>,
    /// Notes créées dans l'ordre de l'import, pour les liaisons et paroles.
    pub first_note_id: Option<usize>,
}

impl Default for MTrack {
    fn default() -> Self {
        Self {
            index: 0,
            index_of_operation: 0,
            program: 0,
            drum_track: false,
            out_channel: 0,
            name: String::new(),
            volumes: BTreeMap::new(),
            pan: None,
            events: Vec::new(),
            chords: ChordMap::new(),
            tuplets: BTreeMap::new(),
            division: 480,
            is_division_in_tps: false,
            had_initial_notes: false,
            has_key: false,
            key_list: KeyList::new(),
            staff: None,
            ms_instr: None,
            first_note_id: None,
        }
    }
}

impl MTrack {
    /// Fin de la note la plus longue d'un accord (`maxNoteOffTime`).
    pub fn max_off_time(chord: &MidiChord) -> Fraction {
        chord
            .notes
            .iter()
            .map(|note| note.off_time)
            .max()
            .unwrap_or(Fraction::ZERO)
    }
}

/// Fin de la note la plus courte (`minNoteOffTime`).
pub fn min_note_off_time(notes: &[MidiNote]) -> Fraction {
    notes
        .iter()
        .map(|note| note.off_time)
        .min()
        .unwrap_or(Fraction::ZERO)
}

/// Fin de la note la plus longue (`maxNoteOffTime`).
pub fn max_note_off_time(notes: &[MidiNote]) -> Fraction {
    notes
        .iter()
        .map(|note| note.off_time)
        .max()
        .unwrap_or(Fraction::ZERO)
}

/// Durée de la note la plus courte d'un accord (`minNoteLen`).
pub fn min_note_len(on_time: Fraction, chord: &MidiChord) -> Fraction {
    min_note_off_time(&chord.notes) - on_time
}

/// Durée de la note la plus longue d'un accord (`maxNoteLen`).
pub fn max_note_len(on_time: Fraction, chord: &MidiChord) -> Fraction {
    max_note_off_time(&chord.notes) - on_time
}

/// Vrai si le programme correspond à un instrument « grand staff »
/// (`MChord::isGrandStaffProgram`).
///
/// La table reprend les programmes de `importmidi_chord.cpp` : pianos, claviers,
/// orgues, harpe, cordes synthétiques, cuivres synthétiques, leads et pads.
pub fn is_grand_staff_program(program: u8) -> bool {
    matches!(
        program,
        0..=7      // pianos, clavecin, clavi
        | 8..=15   // celesta, glockenspiel, boîte à musique, vibraphone, marimba, xylophone, cloches, dulcimer
        | 16..=23  // orgues et accordéons
        | 46       // harpe
        | 50..=52  // cordes synthétiques, voix synthétique
        | 62..=63  // cuivres synthétiques
        | 80..=95  // leads
        | 96..=103 // pads
        | 104..=111 // effets
    )
}

/// Supprime les notes de même hauteur qui se chevauchent dans un accord
/// (`removeOverlappingNotes(QList<MidiNote>&)`).
fn remove_overlapping_notes_in_chord(notes: &mut Vec<MidiNote>) {
    let mut kept: Vec<MidiNote> = Vec::with_capacity(notes.len());
    for note in notes.drain(..) {
        let mut merged = false;
        for existing in &mut kept {
            if existing.pitch == note.pitch {
                if note.off_time > existing.off_time {
                    existing.off_time = note.off_time;
                }
                merged = true;
                break;
            }
        }
        if !merged {
            kept.push(note);
        }
    }
    *notes = kept;
}

/// Supprime les chevauchements de notes de même hauteur et de même voix
/// (`MChord::removeOverlappingNotes`).
pub fn remove_overlapping_notes(track: &mut MTrack) {
    if track.chords.is_empty() {
        return;
    }
    let keys = track.chords.keys();
    for (position, key) in keys.iter().enumerate() {
        let (on_time1, _) = *key;
        let mut remove_chord = false;
        {
            let Some(chord1) = track.chords.get_mut(*key) else {
                continue;
            };
            remove_overlapping_notes_in_chord(&mut chord1.notes);
            if chord1.notes.is_empty() {
                remove_chord = true;
            }
        }
        if remove_chord {
            track.chords.remove(*key);
            continue;
        }

        // Parcours par index : une note trop courte est retirée sans avancer
        // l'index, il faut donc relire la longueur à chaque tour. Un compteur
        // figé paniquait quand la dernière note disparaissait (fichiers Lakh
        // denses, auparavant masqués par le dépassement de 10 s).
        let mut index = 0;
        while let Some((pitch, off_time, voice)) = track.chords.get(*key).and_then(|chord1| {
            chord1
                .notes
                .get(index)
                .map(|note| (note.pitch, note.off_time, chord1.voice))
        }) {
            let mut new_off = off_time;
            let mut tuplet = None;
            let mut is_in_tuplet = false;
            if let Some(chord1) = track.chords.get(*key) {
                if let Some(note) = chord1.notes.get(index) {
                    is_in_tuplet = note.is_in_tuplet;
                    tuplet = note.tuplet;
                }
            }

            for later_key in keys.iter().skip(position + 1) {
                let (on_time2, _) = *later_key;
                if on_time2 >= new_off {
                    break;
                }
                let Some(chord2) = track.chords.get(*later_key) else {
                    continue;
                };
                if chord2.voice != voice {
                    continue;
                }
                if !chord2.notes.iter().any(|note2| note2.pitch == pitch) {
                    continue;
                }
                new_off = on_time2;
                if !is_in_tuplet && chord2.is_in_tuplet {
                    if let Some(tuplet_key) = chord2.tuplet {
                        is_in_tuplet = true;
                        tuplet = Some(tuplet_key);
                    }
                } else if is_in_tuplet && !chord2.is_in_tuplet {
                    is_in_tuplet = false;
                }
                break;
            }

            if new_off - on_time1 < min_allowed_duration() {
                if let Some(chord1) = track.chords.get_mut(*key) {
                    chord1.notes.remove(index);
                }
                continue;
            }
            if let Some(chord1) = track.chords.get_mut(*key) {
                if let Some(note) = chord1.notes.get_mut(index) {
                    note.off_time = new_off;
                    note.is_in_tuplet = is_in_tuplet;
                    note.tuplet = tuplet;
                }
            }
            index += 1;
        }
        let empty = track
            .chords
            .get(*key)
            .is_none_or(|chord1| chord1.notes.is_empty());
        if empty {
            track.chords.remove(*key);
        }
    }
    super::tuplet::remove_empty_tuplets(track);
}

/// Vrai si deux accords partagent une hauteur (`hasNotesWithEqualPitch`).
fn has_notes_with_equal_pitch(chord1: &MidiChord, chord2: &MidiChord) -> bool {
    chord1
        .notes
        .iter()
        .any(|note1| chord2.notes.iter().any(|note2| note2.pitch == note1.pitch))
}

/// Rassemble en accords les notes attaquées presque ensemble
/// (`MChord::collectChords`, algorithme *quickthresh*).
///
/// `human_tol_coeff` et `non_human_tol_coeff` s'appliquent à
/// [`min_allowed_duration`] selon `is_human_performance`.
pub fn collect_chords(
    track: &mut MTrack,
    human_tol_coeff: Fraction,
    non_human_tol_coeff: Fraction,
    is_human: bool,
) {
    if track.chords.is_empty() {
        return;
    }
    let min_allowed = min_allowed_duration();
    let thresh_time = min_allowed
        * if is_human {
            human_tol_coeff
        } else {
            non_human_tol_coeff
        };
    let fudge_time = thresh_time * Fraction::new(1, 4);
    let thresh_ext_time = thresh_time * Fraction::HALF;

    let keys = track.chords.keys();
    let mut previous_key: Option<ChordKey> = None;
    let mut current_chord_start: Option<Fraction> = None;
    let mut current_thresh = thresh_time;
    let mut max_off_time = Fraction::ZERO;
    let mut removed: Vec<ChordKey> = Vec::new();

    for key in keys {
        let (on_time, _) = key;
        let Some((is_in_tuplet, chord_max_off, voice)) = track
            .chords
            .get(key)
            .map(|chord| (chord.is_in_tuplet, MTrack::max_off_time(chord), chord.voice))
        else {
            continue;
        };
        if is_in_tuplet {
            current_chord_start = None;
            previous_key = Some(key);
            continue;
        }

        if let Some(start) = current_chord_start {
            let is_close =
                on_time < start + current_thresh && on_time <= max_off_time - min_allowed;
            if let Some(previous) = previous_key.filter(|_| is_close) {
                let previous_voice = track
                    .chords
                    .get(previous)
                    .map_or(voice, |chord| chord.voice);
                if voice != previous_voice {
                    current_chord_start = None;
                    previous_key = Some(key);
                    continue;
                }
                let add = match (track.chords.get(previous), track.chords.get(key)) {
                    (Some(previous_chord), Some(current_chord)) => {
                        !has_notes_with_equal_pitch(previous_chord, current_chord)
                    }
                    _ => false,
                };
                if add {
                    let notes = track
                        .chords
                        .get(key)
                        .map(|chord| chord.notes.clone())
                        .unwrap_or_default();
                    if let Some(previous_chord) = track.chords.get_mut(previous) {
                        previous_chord.notes.extend(notes);
                    }
                    if chord_max_off > max_off_time {
                        max_off_time = chord_max_off;
                    }
                }
                if on_time >= start + current_thresh - fudge_time && current_thresh == thresh_time {
                    current_thresh += thresh_ext_time;
                }
                removed.push(key);
                continue;
            }
        }

        current_chord_start = Some(on_time);
        max_off_time = chord_max_off;
        current_thresh = thresh_time;
        previous_key = Some(key);
    }

    for key in removed {
        track.chords.remove(key);
    }
}

/// Trie les notes de chaque accord par hauteur (`sortNotesByPitch`).
pub fn sort_notes_by_pitch(track: &mut MTrack) {
    for (_, chord) in track.chords.iter_mut() {
        chord.notes.sort_by_key(|note| note.pitch);
    }
}

/// Trie les notes de chaque accord par durée (`sortNotesByLength`).
pub fn sort_notes_by_length(track: &mut MTrack) {
    for (_, chord) in track.chords.iter_mut() {
        chord.notes.sort_by_key(|note| note.off_time);
    }
}

/// Découpe les accords dont les notes n'ont pas toutes la même durée
/// (`MChord::splitUnequalChords`).
pub fn split_unequal_chords(track: &mut MTrack) {
    if track.chords.is_empty() {
        return;
    }
    sort_notes_by_length(track);
    let keys = track.chords.keys();
    for key in keys {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        let base = chord.clone();
        let mut groups: Vec<Vec<MidiNote>> = Vec::new();
        let mut current: Vec<MidiNote> = Vec::new();
        let mut current_off: Option<Fraction> = None;
        for note in &base.notes {
            match current_off {
                Some(off) if note.off_time == off => current.push(note.clone()),
                _ => {
                    if !current.is_empty() {
                        groups.push(std::mem::take(&mut current));
                    }
                    current_off = Some(note.off_time);
                    current.push(note.clone());
                }
            }
        }
        if !current.is_empty() {
            groups.push(current);
        }
        if groups.len() <= 1 {
            continue;
        }
        let (on_time, _) = key;
        for group in groups.iter().skip(1) {
            let mut new_chord = base.clone();
            new_chord.notes = group.clone();
            track.chords.insert(on_time, new_chord);
        }
        if let Some(chord) = track.chords.get_mut(key) {
            chord.notes = groups[0].clone();
        }
    }
}

/// Durée minimale parmi les notes restantes (`MChord::findMinDuration`).
pub fn find_min_duration(on_time: Fraction, chords: &[&MidiChord], length: Fraction) -> Fraction {
    let mut len = length;
    for chord in chords {
        for note in &chord.notes {
            let duration = note.off_time - on_time;
            if duration < len && duration != Fraction::ZERO {
                len = duration;
            }
        }
    }
    len
}

/// Fusionne les accords de même attaque et même voix
/// (`MChord::mergeChordsWithEqualOnTimeAndVoice`).
pub fn merge_chords_with_equal_on_time_and_voice(track: &mut MTrack) {
    if track.chords.is_empty() {
        return;
    }
    let keys = track.chords.keys();
    let mut seen: BTreeMap<(Fraction, usize), ChordKey> = BTreeMap::new();
    let mut removed = Vec::new();
    for key in keys {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        let slot = (key.0, chord.voice);
        match seen.get(&slot).copied() {
            Some(existing) => {
                let notes = track
                    .chords
                    .get(key)
                    .map(|chord| chord.notes.clone())
                    .unwrap_or_default();
                if let Some(target) = track.chords.get_mut(existing) {
                    target.notes.extend(notes);
                }
                removed.push(key);
            }
            None => {
                seen.insert(slot, key);
            }
        }
    }
    for key in removed {
        track.chords.remove(key);
    }
}

/// Hauteur moyenne d'un accord (`MChord::chordAveragePitch`).
pub fn chord_average_pitch(chord: &MidiChord) -> i32 {
    if chord.notes.is_empty() {
        return 0;
    }
    let sum: i32 = chord.notes.iter().map(|note| note.pitch).sum();
    (f64::from(sum) / chord.notes.len() as f64).round() as i32
}

/// Durée du plus long accord (`MChord::findMaxChordLength`).
pub fn find_max_chord_length(track: &MTrack) -> Fraction {
    let mut max = Fraction::ZERO;
    for (on_time, chord) in track.chords.iter() {
        let length = MTrack::max_off_time(chord) - on_time;
        if length > max {
            max = length;
        }
    }
    max
}

/// Accords d'une voix qui recoupent un intervalle donné
/// (`MChord::findChordsForTimeRange`).
pub fn find_chords_for_time_range(
    track: &MTrack,
    voice: usize,
    on_time: Fraction,
    off_time: Fraction,
    max_chord_length: Fraction,
) -> Vec<ChordKey> {
    let mut result = Vec::new();
    for key in track.chords.keys().into_iter().rev() {
        let Some(chord) = track.chords.get(key) else {
            continue;
        };
        if key.0 >= off_time {
            continue;
        }
        if key.0 + max_chord_length <= on_time {
            break;
        }
        if chord.voice != voice {
            continue;
        }
        let chord_off = MTrack::max_off_time(chord);
        if chord_off > on_time && key.0 < off_time {
            result.push(key);
        }
    }
    result
}

/// Fixe l'index de mesure de chaque accord (`MChord::setBarIndexes`).
pub fn set_bar_indexes(
    track: &mut MTrack,
    basic_quant: Fraction,
    last_tick: Fraction,
    sigmap: &TimeSigMap,
) {
    if track.chords.is_empty() {
        return;
    }
    let keys = track.chords.keys();
    let mut position = 0;
    let mut bar_index: i64 = 0;
    loop {
        let end_bar_tick =
            Fraction::from_ticks(sigmap.bar2tick(bar_index + 1, 0), UNITS_PER_WHOLE as u32);
        if position >= keys.len() {
            break;
        }
        if end_bar_tick <= keys[position].0 {
            bar_index += 1;
            continue;
        }
        while position < keys.len() {
            let key = keys[position];
            let on_time = super::quant::find_quantized_chord_on_time(
                key.0,
                track.chords.get(key).expect("accord présent"),
                basic_quant,
            );
            if on_time >= end_bar_tick {
                break;
            }
            if let Some(chord) = track.chords.get_mut(key) {
                chord.bar_index = bar_index as i32;
            }
            position += 1;
        }
        if position >= keys.len() || end_bar_tick > last_tick {
            break;
        }
        bar_index += 1;
    }
}
