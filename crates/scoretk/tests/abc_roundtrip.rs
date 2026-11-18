// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les airs ci-dessous sont des mélodies traditionnelles du domaine public
// (France, Angleterre), transcrites pour le test. Aucune source externe n'est
// copiée.
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

//! Aller-retour ABC sur les airs écrits à la main.
//!
//! Lit un air, l'écrit en ABC, le relit et vérifie que la structure et les
//! hauteurs sont stables. Ces tests exercent les deux moitiés
//! (`scoretk-abc-import` et `scoretk-abc-export`) ; ils vivent donc dans la
//! façade, pas dans un crate de sens.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

use scoretk_core::model::{GraceType, Track};

const AU_CLAIR: &str = "\
%abc-2.1
X:1
T:Au clair de la lune
C:Traditionnel
M:4/4
L:1/4
K:C
C C C D | E E D z |
w: Au clair de la lu- ne
C C C D | E E D z |
w: Mon a- mi Pier- rot
D D D D | E E D |
C C C D | E E D |]
";

const GREENSLEEVES: &str = "\
X:1
T:Greensleeves
M:6/8
L:1/8
K:Am
A2A c2d | e2e e2d | c2B A2B | c2B A2G |
A2A c2d | e2e e2d | c2B A2B | A4 z2 |]
";

const SUR_LE_PONT: &str = "\
X:1
T:Sur le pont d'Avignon
M:2/4
L:1/8
K:G
d2 d2 | B2 B2 | A2 A2 | G4 |
d2 d2 | B2 B2 | A2 A2 | G4 |]
";

const ORNEMENTS: &str = "\
X:1
T:Ornements
M:4/4
L:1/8
K:D
!p!\"D\" (A2B2) [DFA]2 {=c}B2- | B4 z4 |]
";

#[test]
fn agrements_par_note_et_groupe_unique() {
    // Deux groupes `{}` consécutifs sont interdits par la norme : le lecteur
    // les fusionne, et l'écriture doit produire une seule paire d'accolades
    // avec la barre oblique devant chaque acciaccatura.
    let source = "\
X:1
T:Agréments
M:4/4
L:1/8
K:C
{g}{/a}b2 {/g}G4 |]
";
    let score = scoretk_abc_import::read_abc(source).expect("lecture");
    let kinds: Vec<Option<GraceType>> = score.measures[0]
        .segments
        .iter()
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .filter_map(|chord_rest| chord_rest.chord_ref())
        .flat_map(|chord| chord.grace_chords.iter().map(|grace| grace.chord.grace))
        .collect();
    assert_eq!(
        kinds,
        [
            Some(GraceType::Appoggiatura),
            Some(GraceType::Acciaccatura),
            Some(GraceType::Acciaccatura)
        ]
    );

    let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
    assert!(abc.contains("{g /a}"), "{abc}");
    assert!(abc.contains("{/g}"), "{abc}");

    let again = scoretk_abc_import::read_abc(&abc).expect("relecture");
    let kinds: Vec<Option<GraceType>> = again.measures[0]
        .segments
        .iter()
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .filter_map(|chord_rest| chord_rest.chord_ref())
        .flat_map(|chord| chord.grace_chords.iter().map(|grace| grace.chord.grace))
        .collect();
    assert_eq!(
        kinds,
        [
            Some(GraceType::Appoggiatura),
            Some(GraceType::Acciaccatura),
            Some(GraceType::Acciaccatura)
        ]
    );
}

#[test]
fn aller_retour_abc_sur_les_airs() {
    for source in [AU_CLAIR, GREENSLEEVES, SUR_LE_PONT, ORNEMENTS] {
        let first = scoretk_abc_import::read_abc(source).expect("lecture");
        let abc = scoretk_abc_export::write_abc(&first).expect("écriture");
        let second = scoretk_abc_import::read_abc(&abc).expect("relecture");
        assert_eq!(
            first.measures.len(),
            second.measures.len(),
            "nombre de mesures instable"
        );
        assert_eq!(
            first.notes.len(),
            second.notes.len(),
            "nombre de notes instable\n{abc}"
        );
        let first_pitches: Vec<u8> = first.notes.iter().map(|note| note.pitch).collect();
        let second_pitches: Vec<u8> = second.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(first_pitches, second_pitches, "hauteurs instables\n{abc}");
    }
}
