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

//! Symboles égarés dans les accords et les agréments.
//!
//! Port de `abc2xml.py` 269, `chord_note = dec_note | rest | b1` (lignes
//! 383-385) : un accord peut contenir un silence (`z`) ou un marqueur de
//! rythme brisé, et un agrément peut contenir un accord ; aucun ne doit faire
//! rejeter l'air entier.

use scoretk_abc_import::read_abc;

fn pitches(source: &str) -> Vec<u8> {
    let score = read_abc(source).expect("lecture");
    score.notes.iter().map(|note| note.pitch).collect()
}

#[test]
fn accord_avec_silence_et_rythme_brise() {
    // `[C2z2]` : le silence interne est sauté, la note C reste.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\n[Cz] [.Ae>] 2 |]\n";
    assert_eq!(
        pitches(source),
        vec![60, 69, 76],
        "les notes de l'accord sont lues, z et > écartés"
    );
}

#[test]
fn agrament_avec_accord_et_controle() {
    // `{[Ge]}A2` : un accord dans un agrément ne rejette pas l'air.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\n{[Ge]}A2 |]\n";
    assert_eq!(
        pitches(source),
        vec![67, 76, 69],
        "G et e de l'agrément puis A"
    );

    // Cas « non concerné » : un accord ordinaire garde toutes ses notes.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\n[CEG]2 |]\n";
    assert_eq!(pitches(source), vec![60, 64, 67]);
}
