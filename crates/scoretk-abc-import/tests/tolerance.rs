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

//! Caractères tolérés par `abc2xml.py` mais qui faisaient rejeter l'air entier.
//!
//! Port de `abc2xml.py` 269 : grammaire `errors` (ligne 431, `pas(r'[^:&|]')`
//! avec `errorWarn`), `tie = oneOf('.- -')` (ligne 380), `basenote` avec `y`
//! comme espacement (lignes 367/549), `chord_note` avec décorations (ligne 385),
//! et branche `K:` de `doHeaderField` (lignes 1808-1851).

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Fraction, NoteId, Score, StaffId, Track};

/// Hauteurs et durées des événements du modèle.
fn content(score: &Score) -> (Vec<u8>, Vec<f64>) {
    let track = Track::new(StaffId::from_index(0), 0);
    let mut pitches = Vec::new();
    let mut durations = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(chord_rest) = segment.chord_rest(track) {
                if let Some(chord) = chord_rest.chord_ref() {
                    pitches.extend(chord.notes.iter().map(|id| score.note(*id).pitch));
                }
                durations.push(chord_rest.duration.ticks().as_f64());
            }
        }
    }
    (pitches, durations)
}

/// Liaisons séparées de la note par un rythme brisé ou un espace, `y`,
/// virgule égarée, décorations dans un accord et numéro de volta après `[|]`.
#[test]
fn caracteres_recuperes_dans_la_musique() {
    let source = "\
X:1
M:4/4
L:1/8
K:C
A>-A B2 | G4 - G8 | ,C D y E F | [C(E]2 z2 | [|]1 c |
";
    let score = read_abc(source).expect("l'air doit être lu, pas rejeté");
    assert_eq!(score.measures.len(), 5, "cinq mesures écrites");
    assert_eq!(
        content(&score),
        (
            vec![69, 69, 71, 67, 67, 60, 62, 64, 65, 60, 64, 72],
            vec![0.1875, 0.0625, 0.25, 0.5, 1.0, 0.125, 0.125, 0.125, 0.125, 0.25, 0.25, 0.125]
        ),
        "hauteurs et durées exactes, y étant un espacement sans note"
    );
    // Les deux tirets sont des liaisons, pas des erreurs : ils relient les
    // deux `A` et les deux `G` (la seconde note porte le `tie_back`).
    assert!(score.note(NoteId::from_index(1)).tie_back.is_some());
    assert!(score.note(NoteId::from_index(4)).tie_back.is_some());
    assert!(score.note(NoteId::from_index(0)).tie_back.is_none());
    // La virgule égarée devant `C` n'a pas décalé l'octave.
    assert_eq!(score.note(NoteId::from_index(5)).pitch, 60);
}

/// Formes de `K:` non reconnues auparavant, mesure additive `2+3/8`, et
/// cas « non concerné » : un air ordinaire reste identique.
#[test]
fn en_tete_toleree_et_controle() {
    let source = "X:1\nM:4/4\nL:1/8\nK:Bn\nC D E F | G A B c |]\n";
    assert_eq!(
        content(&read_abc(source).expect("K:Bn")),
        (vec![61, 63, 64, 66, 68, 70, 71, 73], vec![0.125; 8]),
        " suffixe inconnu : ton majeur (port de la ligne 1817)"
    );

    let source = "X:1\nM:4/4\nL:1/8\nK:Dmixm\nC D E F | G A B c |]\n";
    assert_eq!(
        content(&read_abc(source).expect("K:Dmixm")),
        (vec![60, 62, 64, 66, 67, 69, 71, 72], vec![0.125; 8]),
        " trois premières lettres du mode : D mixolydien (1 dièse, F#)"
    );

    let source = "X:1\nM:4/4\nL:1/8\nK:treble+8\nC D E F |]\n";
    assert_eq!(
        content(&read_abc(source).expect("K:treble+8")),
        (vec![72, 74, 76, 77], vec![0.125; 4]),
        " K:treble+8 : armure par défaut, mais hauteurs d'un octave au-dessus (doClef, abc2xml.py:1736-1744)"
    );

    let source = "X:1\nM:2+3/8\nL:1/8\nK:C\nC D E F G |]\n";
    let score = read_abc(source).expect("M:2+3/8");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].nominal_len, Fraction::new(5, 8));
    assert_eq!(
        content(&score),
        (vec![60, 62, 64, 65, 67], vec![0.125; 5]),
        " mesure additive : 2+3 = 8èmes"
    );

    // Cas « non concerné » : l'air ordinaire garde exactement sa lecture.
    let source = "X:1\nM:4/4\nL:1/8\nK:C\nC D E F | G A B c |]\n";
    assert_eq!(
        content(&read_abc(source).expect("contrôle")),
        (vec![60, 62, 64, 65, 67, 69, 71, 72], vec![0.125; 8])
    );
}

/// Ligne d'en-tête sans champ avant `K:`, et altération en fin de ligne
/// continuée sur la note suivante (deux causes d'airs rejetés dans le corpus).
#[test]
fn header_sans_champ_et_alteration_continuee() {
    // `3/8=120` n'a pas de champ `Q:` : la ligne est sautée, l'air est lu.
    let source = "X:1\nM:3/8\nL:1/8\n3/8=120\nI: :: ::\nK:D\nf/e/|d2A|]\n";
    let score = read_abc(source).expect("tempo nu avant K:");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        content(&score),
        (vec![78, 76, 74, 69], vec![0.0625, 0.0625, 0.25, 0.125])
    );

    // En `K:G`, `=` en fin de ligne naturalise le `f` de la ligne suivante
    // (F# → F), exactement comme si les deux lignes n'en faisaient qu'une.
    let source = "X:1\nM:4/4\nL:1/8\nK:G\nC D E =\nf2 |]\n";
    let score = read_abc(source).expect("altération en fin de ligne");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        content(&score),
        (vec![60, 62, 64, 77], vec![0.125, 0.125, 0.125, 0.25])
    );
}
