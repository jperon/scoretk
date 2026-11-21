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

//! Réduction du test amont `tuplet_nonuplet_3-4` avec les **options par
//! défaut** : la détection littérale doit être active sans drapeau, à
//! l'étape précoce, pour retrouver les attaques de la référence MuseScore.

mod common;

use scoretk_core::model::{ChordRestKind, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{smf, Ev};

/// Notes `(hauteur, attaque en noires)` ramenées sur la grille Q16, avec
/// fusion des chaînes de liaisons (comme `scripts/oracle/midi/import_mesure.py`).
fn notes(score: &Score) -> Vec<(u8, f64)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                let onset =
                    (segment.tick.numerator() as f64 / segment.tick.denominator() as f64) * 4.0;
                let onset = (onset * 4.0).round() / 4.0;
                for note_id in &chord.notes {
                    if score.notes[note_id.index()].tie_back.is_some() {
                        continue;
                    }
                    result.push((score.notes[note_id.index()].pitch, onset));
                }
            }
        }
    }
    result.sort_by(|left, right| left.1.total_cmp(&right.1).then(left.0.cmp(&right.0)));
    result
}

/// `tuplet_nonuplet_3-4.mid` : 3/4, tempo 500 000, 72 à 960 (479 ticks),
/// puis 71, 72, 74, 76, 77, 76, 74, 72, 71 toutes les 53 ticks (52 ticks).
fn score_avec_grille_sans_n_olet() -> Vec<u8> {
    let meta = vec![(0, Ev::TimeSig(3, 4)), (0, Ev::Tempo(500_000))];
    let mut events = vec![(960, Ev::NoteOn(0, 72, 80)), (1439, Ev::NoteOff(0, 72))];
    for (index, pitch) in [71, 72, 74, 76, 77, 76, 74, 72, 71].into_iter().enumerate() {
        let onset = 1440 + 53 * index as u32;
        events.push((onset, Ev::NoteOn(0, pitch, 80)));
        events.push((onset + 52, Ev::NoteOff(0, pitch)));
    }
    events.sort_by_key(|(tick, _)| *tick);
    smf(&[meta, events])
}

#[test]
fn detection_litterale_est_activee_par_defaut() {
    let score = read_midi(&score_avec_grille_sans_n_olet()).expect("lecture");
    // Référence MuseScore 4.7.4 (tuplet_nonuplet_3-4-ref.mscx).
    let reference = vec![
        (72, 2.0),
        (71, 3.0),
        (72, 3.0),
        (74, 3.25),
        (76, 3.25),
        (76, 3.5),
        (77, 3.5),
        (72, 3.75),
        (74, 3.75),
        (71, 4.0),
    ];
    assert_eq!(notes(&score), reference);
}
