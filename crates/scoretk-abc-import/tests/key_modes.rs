// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. Les airs sont écrits à la main pour l'occasion.
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

//! Noms de mode séparés du nom de tonique dans `K:` (`K:E Minor`, `K:A Dorian`…).
//!
//! Port de `abc2xml.py` 269, `doHeaderField`, branche `K` (lignes 1808-1850) :
//! la regex `([A-G][#b]?)\s*([a-zA-Z]*)` lit la tonique **et** le mot de mode
//! qui la suit, même séparé par un espace. Le lecteur ne doit pas confondre
//! `K:E Minor` (1 dièse) avec `K:E` majeur (4 dièses).

use scoretk_abc_import::read_abc;

/// Hauteurs et durées des notes d'un air, avec le nombre de mesures.
fn content(source: &str) -> (usize, Vec<u8>, Vec<f64>) {
    let score = read_abc(source).expect("lecture");
    let pitches = score.notes.iter().map(|note| note.pitch).collect();
    let durations = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| {
            segment.chord_rest(scoretk_core::model::Track::new(
                scoretk_core::model::StaffId::from_index(0),
                0,
            ))
        })
        .map(|chord_rest| chord_rest.duration.ticks().as_f64())
        .collect();
    (score.measures.len(), pitches, durations)
}

const MUSIC: &str = "C D E F | G A B c |]";

/// Le mot de mode est séparé de la tonique : l'armure doit être celle du
/// mode, pas celle de la tonique prise seule (majeure).
#[test]
fn mode_separe_par_un_espace() {
    // Mineure relative : E mineur = 1 dièse (F#), pas E majeur = 4 dièses.
    let source = format!("X:1\nM:4/4\nL:1/8\nK:E Minor\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![60, 62, 64, 66, 67, 69, 71, 72], vec![0.125; 8],),
        "K:E Minor : F dièse, C et G naturels"
    );

    // Dorien : A dorien = 1 dièse (F#), pas A majeur = 3 dièses.
    let source = format!("X:1\nM:4/4\nL:1/8\nK:A Dorian\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![60, 62, 64, 66, 67, 69, 71, 72], vec![0.125; 8]),
        "K:A Dorian : seul F est dièse"
    );

    // Mixolydien : A mixolydien = 2 dièses (F#, C#), G reste naturel.
    let source = format!("X:1\nM:4/4\nL:1/8\nK:A Mixolydian\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![61, 62, 64, 66, 67, 69, 71, 73], vec![0.125; 8]),
        "K:A Mixolydian : F# et C#, G naturel"
    );
}

/// Abréviations et cas « non concerné » : les tons majeurs et les formes
/// déjà attachées à la tonique gardent exactement la même lecture.
#[test]
fn abreviations_et_controle() {
    let source = format!("X:1\nM:4/4\nL:1/8\nK:A min\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![60, 62, 64, 65, 67, 69, 71, 72], vec![0.125; 8]),
        "K:A min : A mineur n'a aucune altération"
    );

    let source = format!("X:1\nM:4/4\nL:1/8\nK:A mix\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![61, 62, 64, 66, 67, 69, 71, 73], vec![0.125; 8]),
        "K:A mix : deux dièses"
    );

    // Cas limite : le mot de mode est attaché, sans espace, et un changement
    // de voix survient après l'en-tête.
    let source = format!("X:1\nM:4/4\nL:1/8\nK:EMinor\nV:1\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![60, 62, 64, 66, 67, 69, 71, 72], vec![0.125; 8]),
        "K:EMinor : forme attachée inchangée"
    );

    // Cas « non concerné » : ton majeur seul, sans mot de mode.
    let source = format!("X:1\nM:4/4\nL:1/8\nK:C\n{MUSIC}");
    assert_eq!(
        content(&source),
        (2, vec![60, 62, 64, 65, 67, 69, 71, 72], vec![0.125; 8]),
        "K:C : ton majeur, aucune altération"
    );
}
