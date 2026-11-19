// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_key.cpp` et `importmidi_key.h` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026. L'algorithme de détection est celui de la thèse de Jürgen Kilian,
// « Inferring Score Level Musical Information From Low-Level Musical Data »,
// 2004, comme dans le C++.
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

//! Armures : liste par portée, détection de l'armure principale.
//!
//! Portage de `MidiKey` (`importmidi_key.cpp`) : la détection compte les
//! demi-tons de passage entre accords successifs, puis choisit l'armure dont
//! les deux degrés sont les plus fréquents.

use std::collections::BTreeMap;

use scoretk_core::model::{
    Fraction, Key, KeyMode, KeySig, Score, Segment, SegmentItem, SegmentType, StaffId, Track,
};

use super::chord::MTrack;
use super::midifile::UNITS_PER_WHOLE;

/// Armure d'une piste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackKey {
    /// Armure concert.
    pub concert_key: Key,
    /// Armure écrite (identique ici : l'import MIDI ne transpose pas).
    pub key: Key,
    /// Mode lu dans le drapeau `mi` du méta `FF 59` ; `Unknown` pour une
    /// armure déduite des notes, comme dans l'import MuseScore.
    pub mode: KeyMode,
}

/// Liste d'armures par tick (`KeyList` de `engraving/dom/keylist.h`).
#[derive(Clone, Debug, Default)]
pub struct KeyList {
    entries: BTreeMap<Fraction, TrackKey>,
}

impl KeyList {
    /// Construit une liste vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Vrai si la liste est vide.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Enregistre une armure au tick donné (`KeyList::setKey`).
    pub fn set_key(&mut self, tick: Fraction, key: TrackKey) {
        self.entries.insert(tick, key);
    }

    /// Armure au tick donné, s'il y en a une.
    pub fn get(&self, tick: Fraction) -> Option<TrackKey> {
        self.entries.get(&tick).copied()
    }

    /// Itère sur les armures dans l'ordre des ticks.
    pub fn iter(&self) -> impl Iterator<Item = (Fraction, TrackKey)> + '_ {
        self.entries.iter().map(|(tick, key)| (*tick, *key))
    }
}

/// Détecte l'armure principale de l'ensemble des pistes (`MidiKey::findKey`).
///
/// Compte les transitions d'un demi-ton entre notes d'accords successifs,
/// puis note chaque armure par la somme des deux classes de hauteur qui la
/// caractérisent ; à égalité, l'armure au moins d'altérations gagne.
pub fn find_key(tracks: &[MTrack]) -> Key {
    let octave = 12;
    let mut counts = [0_i64; 12];

    for track in tracks {
        if track.drum_track {
            continue;
        }
        let keys = track.chords.keys();
        for window in keys.windows(2) {
            let (Some(chord1), Some(chord2)) =
                (track.chords.get(window[0]), track.chords.get(window[1]))
            else {
                continue;
            };
            for note1 in &chord1.notes {
                for note2 in &chord2.notes {
                    if (note1.pitch - note2.pitch).abs() == 1 {
                        let low = note1.pitch.min(note2.pitch).rem_euclid(octave);
                        counts[low as usize] += 1;
                    }
                }
            }
        }
    }

    let index = |value: usize| value % octave as usize;
    let keys = [
        (Key::C_FLAT, counts[index(3)] + counts[index(10)]),
        (Key::G_FLAT, counts[index(10)] + counts[index(5)]),
        (Key::D_FLAT, counts[index(5)] + counts[index(0)]),
        (Key::A_FLAT, counts[index(0)] + counts[index(7)]),
        (Key::E_FLAT, counts[index(7)] + counts[index(2)]),
        (Key::B_FLAT, counts[index(2)] + counts[index(9)]),
        (Key::F, counts[index(9)] + counts[index(4)]),
        (Key::C, counts[index(4)] + counts[index(11)]),
        (Key::G, counts[index(11)] + counts[index(6)]),
        (Key::D, counts[index(6)] + counts[index(1)]),
        (Key::A, counts[index(1)] + counts[index(8)]),
        (Key::E, counts[index(8)] + counts[index(3)]),
        (Key::B, counts[index(3)] + counts[index(10)]),
        (Key::F_SHARP, counts[index(10)] + counts[index(5)]),
        (Key::C_SHARP, counts[index(5)] + counts[index(0)]),
    ];

    keys.iter()
        .max_by(|left, right| {
            // `KeyData::operator<` : plus grand compte d'abord, puis moins
            // d'altérations.
            left.1
                .cmp(&right.1)
                .then_with(|| right.0.fifths().abs().cmp(&left.0.fifths().abs()))
        })
        .map(|(key, _)| *key)
        .unwrap_or(Key::C)
}

/// Pose une liste d'armures sur une portée (`assignKeyListToStaff`).
///
/// Une armure de do majeur qui suit une armure de do majeur n'est pas écrite,
/// comme dans le C++ (la première, précédée implicitement de do, l'est).
pub fn assign_key_list_to_staff(score: &mut Score, staff: StaffId, key_list: &KeyList) {
    let mut previous = Key::C;
    let mut previous_mode = KeyMode::Unknown;
    let staff_count = score.staves.len();
    for (tick, key) in key_list.iter() {
        if key.key == Key::C && key.key == previous && key.mode == previous_mode {
            continue;
        }
        previous = key.key;
        previous_mode = key.mode;
        let Some(measure_index) = measure_index_for_tick(score, tick) else {
            continue;
        };
        let start = score.measures[measure_index].start_tick;
        let mut keysig = KeySig::new(key.concert_key);
        keysig.transposed_key = key.key;
        // Le mode du SMF est conservé tel quel, comme pour l'import MusicXML
        // (`pass2.rs` : `<mode>minor</mode>` → `KeyMode::Minor`).
        keysig.mode = key.mode;
        let measure = &mut score.measures[measure_index];
        let segment = match measure
            .segments
            .iter_mut()
            .find(|segment| segment.kind == SegmentType::KeySig && segment.tick == start)
        {
            Some(segment) => segment,
            None => {
                measure.add_segment(Segment::new(SegmentType::KeySig, start, staff_count));
                measure
                    .segments
                    .iter_mut()
                    .find(|segment| segment.kind == SegmentType::KeySig && segment.tick == start)
                    .expect("segment d'armure créé")
            }
        };
        segment.set_element(Track::new(staff, 0), SegmentItem::KeySig(keysig));
    }
}

/// Index de la mesure contenant un tick, ou `None` si hors partition.
pub fn measure_index_for_tick(score: &Score, tick: Fraction) -> Option<usize> {
    score
        .measures
        .iter()
        .position(|measure| measure.start_tick <= tick && tick < measure.end_tick())
}

/// Détecte l'armure principale et l'applique aux pistes qui n'en portent pas
/// (`MidiKey::recognizeMainKeySig`).
pub fn recognize_main_key_sig(score: &mut Score, tracks: &mut [MTrack]) {
    let need_to_find_key = tracks
        .iter()
        .any(|track| !track.drum_track && !track.has_key);
    if !need_to_find_key {
        return;
    }
    let key = find_key(tracks);
    for track in tracks.iter_mut() {
        if track.drum_track {
            continue;
        }
        if track.has_key {
            continue;
        }
        let staff = track
            .staff
            .expect("portée créée avant la détection d'armure");
        let mut key_list = track.key_list.clone();
        key_list.set_key(
            Fraction::ZERO,
            TrackKey {
                concert_key: key,
                key,
                mode: KeyMode::Unknown,
            },
        );
        assign_key_list_to_staff(score, staff, &key_list);
    }
}

/// Tick (`Fraction`) d'une position en ticks MuseScore.
pub fn fraction_from_ticks(ticks: i64) -> Fraction {
    Fraction::from_ticks(ticks, UNITS_PER_WHOLE as u32)
}
