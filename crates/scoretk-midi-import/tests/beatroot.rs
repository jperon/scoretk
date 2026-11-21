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

//! Tests réduits de BeatRoot sur des listes d'attaques fabriquées.

use scoretk_midi_import::beatroot::{beat_track, Event};

fn onsets(times: &[f64]) -> Vec<Event> {
    times.iter().map(|time| Event::new(*time, 1.0)).collect()
}

#[test]
fn tempo_induit_sur_une_grille_reguliere() {
    let events = onsets(&[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0]);
    let beats = beat_track(&events);
    assert!(beats.len() >= 6, "trop peu de temps détectés : {beats:?}");
    // Les temps doivent être réguliers à 0,5 s près.
    for pair in beats.windows(2) {
        assert!(
            (pair[1] - pair[0] - 0.5).abs() < 0.1,
            "temps irréguliers : {beats:?}"
        );
    }
}

#[test]
fn peu_d_evenements_ne_donnent_pas_de_tempo() {
    // Sans au moins deux attaques, `Induction::beatInduction` ne produit aucun
    // amas d'intervalles et donc aucune hypothèse de tempo.
    let events = onsets(&[1.0]);
    assert!(beat_track(&events).is_empty());
    let events = onsets(&[0.0, 0.5]);
    assert!(beat_track(&events).len() >= 2);
}
