// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce test est une réécriture Rust originale : aucune ligne de `abc2xml.py`
// n'est copiée. Les airs réduits proviennent du corpus `abcjs-tests` et de
// `music21-corpus` (voir `docs/agents/abc.md` § 16.3/§ 16.4 et § 16.6).
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

//! Placement des attaques avec rythme brisé, n-olets et durées en slashs.
//!
//! Port de `abc2xml.py` : `convertBroken` (ligne 614), `doBroken` (ligne 594),
//! `note_length` (lignes 362/473), `tuplet_start` (lignes 395/2012) et
//! `mkTmod` (ligne 728). Avant le port, le marqueur `<`/`>` allongeait la
//! figure écrite de la note précédente SANS déplacer le curseur : la mesure
//! finissait court et toutes les attaques suivantes étaient en avance.

use scoretk_abc_import::read_abc;
use scoretk_core::model::{Fraction, Score, StaffId, Track};

/// Air réduit de `abcjs-tests/extracted-abc/timing.test__abcWarp.abc` X:1.
const WARP: &str = "\
X:1
T:abcWarp
M:4/4
L:1/4
K:F
CD E>F | (3GAB Ac :|
";

/// Air réduit de `music21-corpus/.../airdsAirs/book1.abc` X:0008 ( Quick Step,
/// 21th Regt. ) : durées en slashs simples et rythme brisé, dans une mesure
/// ordinaire à deux temps avec une reprise.
const QUICK_STEP: &str = "\
X:8
T:Quick Step. 21th. Regt.
M:2/4
L:1/8
K:D
FA A(G/F/)|BA A(B/c/)|d>ef>g|aA A2 :|
";

/// Phrase réduite de `book1.abc` (X:0004 et suivants, `(d/e//f//)`) : slashs
/// DOUBLES, pour vérifier que `note_length` consomme bien tous les `/`.
const DOUBLE_SLASH: &str = "\
X:1
T:Double slashes
M:2/4
L:1/8
K:D
(d/e//f//) c d e | f g a b |]
";

/// (hauteur, durée écrite, attaque absolue) des notes d'une voix, dans
/// l'ordre du fichier. L'attaque est la clé mesurée par l'oracle N2.
fn notes(score: &Score) -> Vec<(u8, Fraction, Fraction)> {
    let track = Track::new(StaffId::from_index(0), 0);
    let mut out = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(chord_rest) = segment.chord_rest(track) {
                if let Some(chord) = chord_rest.chord_ref() {
                    for id in &chord.notes {
                        out.push((
                            score.note(*id).pitch,
                            chord_rest.duration.ticks(),
                            segment.tick,
                        ));
                    }
                }
            }
        }
    }
    out
}

/// Raccourci de lecture : `p` = hauteur MIDI, `d` = durée, `t` = attaque.
fn note(p: u8, n: i64, d: i64, t: i64, td: i64) -> (u8, Fraction, Fraction) {
    (p, Fraction::new(n, d), Fraction::new(t, td))
}

#[test]
fn rythme_brise_deplace_les_attaques_suivantes() {
    let score = read_abc(WARP).expect("lecture de l'air réduit");
    assert_eq!(score.measures.len(), 2, "deux mesures écrites");

    // `E>` vaut une noire pointée (3/8) et `F` une croche (1/8) : la mesure
    // fait toujours 4/4. Le curseur doit suivre la durée CORRIGÉE de `E`,
    // sinon `F` et toute la mesure suivante sont en avance d'un huitième.
    assert_eq!(score.measures[0].len, Fraction::ONE, "mesure 1 = 4/4");
    assert_eq!(
        score.measures[1].start_tick,
        Fraction::ONE,
        "la mesure 2 commence après une mesure pleine"
    );
    assert_eq!(
        notes(&score),
        vec![
            note(60, 1, 4, 0, 1), // C4
            note(62, 1, 4, 1, 4), // D4
            note(64, 3, 8, 1, 2), // E4 pointée
            note(65, 1, 8, 7, 8), // F4
            // (3GAB : trois notes écrites 1/4 (n-olet 3:2, jouées 1/6), puis
            // A et c de 1/4. Le modèle garde la durée NOTÉE ; c'est
            // l'exporteur qui écrit 1/6 via <time-modification>.
            note(67, 1, 4, 1, 1),
            note(69, 1, 4, 7, 6),
            note(70, 1, 4, 4, 3), // B♭4 (armure F)
            note(69, 1, 4, 3, 2),
            note(72, 1, 4, 7, 4),
        ]
    );
}

#[test]
fn slashs_et_rythme_brise_en_musique_ordinaire() {
    let score = read_abc(QUICK_STEP).expect("lecture de l'air réduit");
    assert_eq!(score.measures.len(), 4, "quatre mesures 2/4");
    for measure in &score.measures {
        assert_eq!(measure.len, Fraction::new(1, 2), "mesure 2/4 écrite");
    }
    let n = notes(&score);
    assert_eq!(n.len(), 17, "5 + 5 + 4 + 3 notes");

    // Mesure 1 : F# A A, puis G/ F/ (slashs simples) — attaques 0, 1/8, 1/4,
    // 3/8, 7/16. Mesure 3 : `d>e` puis `f>g`, chaque paire occupe 1/4 :
    // noire pointée (3/16) / croche (1/16), attaques 1, 19/16, 5/4, 23/16.
    assert_eq!(
        &n[..5],
        &[
            note(66, 1, 8, 0, 1),
            note(69, 1, 8, 1, 8),
            note(69, 1, 8, 1, 4),
            note(67, 1, 16, 3, 8),
            note(66, 1, 16, 7, 16),
        ]
    );
    assert_eq!(
        &n[10..14],
        &[
            note(74, 3, 16, 1, 1),   // d>
            note(76, 1, 16, 19, 16), // e
            note(78, 3, 16, 5, 4),   // f>
            note(79, 1, 16, 23, 16), // g
        ]
    );
    // Dernière mesure : la reprise ne modifie ni hauteurs ni durées.
    assert_eq!(n[14], note(81, 1, 8, 3, 2));
    assert_eq!(n[16], note(69, 1, 4, 7, 4));
}

/// Mesure réduite de `music21-corpus/.../airdsAirs/book1.abc` X:0142 :
/// l'accord `[G2G,2]` porte ses durées `2` sur CHAQUE note.
const CHORD_LENGTH: &str = "\
X:142
T:The Dumb Glutton (extrait)
M:2/4
L:1/8
K:G
G2 [G2G,2] |]
";

#[test]
fn longueurs_internes_d_accord_se_multiplient() {
    let score = read_abc(CHORD_LENGTH).expect("lecture de l'air réduit");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        score.measures[0].len,
        Fraction::new(1, 2),
        "mesure 2/4 pleine"
    );
    // Port de `abc2xml.py::convertChord` (ligne 651) : la durée de chaque
    // note d'accord est son `2` multiplié par la durée de l'accord (vide = 1).
    // Avant le port, le `2` interne était ignoré, l'accord valait 1/8 et la
    // mesure n'était pleine qu'au prix d'un `<forward>` compensateur.
    assert_eq!(
        notes(&score),
        vec![
            note(67, 1, 4, 0, 1), // G2
            note(67, 1, 4, 1, 4), // [G2 ...
            note(55, 1, 4, 1, 4), // ... G,2] (même attaque)
        ]
    );
}

#[test]
fn slashs_doubles_sont_des_diviseurs_de_quatre() {
    let score = read_abc(DOUBLE_SLASH).expect("lecture de l'air réduit");
    let n = notes(&score);
    // d/ = 1/16, e// = 1/32, f// = 1/32 : le curseur avance de 1/16, 1/32,
    // 1/32 puis d'un huitième par note. Un seul slash lu donnerait 1/16 pour
    // e et f et décalerait toutes les attaques suivantes.
    assert_eq!(
        &n[..3],
        &[
            note(74, 1, 16, 0, 1),
            note(76, 1, 32, 1, 16),
            note(78, 1, 32, 3, 32),
        ]
    );
    // Cas « non concerné » : la seconde mesure ne contient que des croches.
    assert_eq!(
        &n[6..],
        &[
            note(78, 1, 8, 1, 2),
            note(79, 1, 8, 5, 8),
            note(81, 1, 8, 3, 4),
            note(83, 1, 8, 7, 8),
        ]
    );
}
