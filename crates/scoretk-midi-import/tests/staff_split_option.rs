// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Réduction du test amont `voice_central` : une seule piste de piano dont
//! l'accord initial couvre plus d'une octave. Quand l'option `doStaffSplit`
//! est verrouillée sur `false` (comme dans les tests amont
//! `voiceSeparation`), MuseScore ne crée pas de grande portée : les deux
//! couches rythmiques restent dans la même portée, en voix 1 et 2.

mod common;

use std::collections::BTreeSet;

use scoretk_core::model::{ChordRestKind, SegmentItem};
use scoretk_midi_import::{read_midi_with, ImportOptions};

use common::{smf, Ev};

fn piano_avec_accord_large() -> Vec<u8> {
    let mut events = vec![(0, Ev::TimeSig(4, 4)), (0, Ev::Tempo(500_000))];
    for pitch in [64, 67, 77] {
        events.push((0, Ev::NoteOn(0, pitch, 80)));
        events.push((479, Ev::NoteOff(0, pitch)));
    }
    events.push((0, Ev::NoteOn(0, 72, 80)));
    events.push((959, Ev::NoteOff(0, 72)));
    events.sort_by_key(|(tick, _)| *tick);
    smf(&[events])
}

#[test]
fn option_do_staff_split_verrouillee_garde_une_seule_portee() {
    let options = ImportOptions {
        do_staff_split: false,
        allow_staff_split_redefinition: false,
        ..ImportOptions::default()
    };
    let score = read_midi_with(&piano_avec_accord_large(), &options).expect("lecture");
    assert_eq!(
        score.staves.len(),
        1,
        "une portée seulement quand la séparation est verrouillée à false"
    );
    let mut voices = BTreeSet::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (voice, item) in segment.iter_elements() {
                if matches!(item, SegmentItem::ChordRest(chord_rest)
                    if matches!(chord_rest.kind, ChordRestKind::Chord(_)))
                {
                    voices.insert(voice.raw());
                }
            }
        }
    }
    assert_eq!(
        voices.len(),
        2,
        "deux voix dans la même portée : {voices:?}"
    );
}
