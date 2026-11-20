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

#[test]
fn volta_avec_texte_entre_crochets() {
    // Port de `abc2xml.py`, `volta_text = Suppress('[') + pas(r'"[^"]+"')`
    // (ligne 417) et `bar_left` (ligne 424) : la norme ABC 2.1 § 4.9 est
    // muette sur les fins de reprise étiquetées, mais le script accepte
    // `["Coda"` comme une volta textuelle. Le lecteur doit consommer tout le
    // texte entre guillemets, sinon le chiffre ou la lettre suivante est pris
    // pour une hauteur et l'air entier est rejeté.
    let source = "\
X:1
M:4/4
L:1/8
K:C
c2 [CEG]2 | e2 (\"4\"c'3.B) | [\"Coda\" e2 (\"4\"c'3.B) (dc3)c2 |]
";
    let score = read_abc(source).expect("la volta textuelle ne rejette pas l'air");
    let mut notes = Vec::new();
    let mut durations = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(chord_rest) = segment.chord_rest(scoretk_core::model::Track::new(
                scoretk_core::model::StaffId::from_index(0),
                0,
            )) {
                if let Some(chord) = chord_rest.chord_ref() {
                    notes.extend(chord.notes.iter().map(|id| score.note(*id).pitch));
                }
                durations.push(chord_rest.duration.ticks().as_f64());
            }
        }
    }
    assert_eq!(score.measures.len(), 3, "trois mesures écrites");
    assert_eq!(
        notes,
        vec![72, 60, 64, 67, 76, 84, 71, 76, 84, 71, 74, 72, 72],
        "hauteurs : accord ordinaire, slur/annotation, puis notes après la volta"
    );
    assert_eq!(
        durations,
        vec![0.25, 0.25, 0.25, 0.375, 0.125, 0.25, 0.375, 0.125, 0.125, 0.375, 0.25],
        "durées écrites (L:1/8) de chaque chord/rest"
    );

    // Cas limite : la volta textuelle en fin de fichier, sans note après.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nC2 | [\"Coda\"\n";
    let score = read_abc(source).expect("volta textuelle en fin de fichier");
    assert_eq!(
        score.notes.iter().map(|note| note.pitch).collect::<Vec<_>>(),
        vec![60],
        "la note avant la volta reste seule"
    );
}
