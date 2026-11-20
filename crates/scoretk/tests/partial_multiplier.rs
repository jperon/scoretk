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

//! Levées avec multiplicateur (`\partial 8*5`) lues depuis la façade.
//!
//! Notation Reference §2.3.3 (Upbeats) : `\partial durée` donne la longueur
//! de la musique qui précède la première barre ; §2.1.3 (Scaling durations) :
//! le facteur `*N/M` fait partie de la durée. Sources réduites de trois
//! partitions du lot `ly` que MuseScore 4.7.4 refusait d'ouvrir
//! (`lily-84610acc.ly`, `lily-c9f61d7f.ly`, `lily-8af97c94.ly`).

#![cfg(feature = "lilypond-import")]

mod common;

use scoretk::{read, Format};
use scoretk_core::model::{Fraction, Track};

use common::consistency::assert_mesures_coherentes;

fn lire(source: &str) -> scoretk_core::model::Score {
    read(source.as_bytes(), Format::Ly).unwrap_or_else(|error| panic!("lecture : {error}"))
}

/// Hauteurs d'une piste, mesure par mesure.
fn hauteurs_piste(score: &scoretk_core::model::Score, piste: u32) -> Vec<Vec<u8>> {
    let piste = Track::from_raw(piste);
    score
        .measures
        .iter()
        .map(|mesure| {
            mesure
                .segments
                .iter()
                .filter_map(|segment| segment.chord_rest(piste))
                .flat_map(|chord_rest| {
                    chord_rest.chord_ref().map_or(Vec::new(), |chord| {
                        chord
                            .notes
                            .iter()
                            .map(|id| score.note(*id).pitch)
                            .collect::<Vec<_>>()
                    })
                })
                .collect()
        })
        .collect()
}

#[test]
fn partial_multiplicateur_levee_et_mesures_suivantes() {
    // Deux portées, variables, `\relative`, accords, silences, barres de
    // contrôle et une seconde levée en fin de morceau — comme les sources
    // réelles. Un `\score` ultérieur avec `\partial 4*4` ne doit pas affecter
    // la partition gravée (premier `\score` retenu, cf. docs/agents/lilypond.md §7).
    let source = "\
\\version \"2.24.0\"\n\\language \"english\"\n\
haut = \\relative c'' { \
\\time 6/8 \\key g \\major \\clef treble \
\\partial 8*5 c8 d8 <e g>8 r8 c8 | \
c4. d4 r8 | \
\\partial 8*5 e4. r4 \\bar \"|.\" \
}\n\
bas = \\fixed c { \
\\time 6/8 \\key g \\major \\clef bass \
\\partial 8*5 c8 g,8 c8 e8 g8 | \
c4. g,4 r8 | \
\\partial 8*5 c4. r4 \\bar \"|.\" \
}\n\
\\score { << \\new Staff << \\new Voice = \"haut\" \\haut >> \\new Staff \\bas >> \\layout {} }\n\
\\score { \\new Staff { \\fixed c' { \\partial 4*4 c1 } } }\n";
    let score = lire(source);
    assert_eq!(score.measures.len(), 3);
    // Levée de cinq croches, puis une mesure pleine en 6/8, puis une dernière
    // mesure raccourcie à cinq croches par le `\partial` de fin.
    let attendu = [
        Fraction::new(5, 8),
        Fraction::new(3, 4),
        Fraction::new(5, 8),
    ];
    for (index, mesure) in score.measures.iter().enumerate() {
        assert_eq!(mesure.len, attendu[index], "mesure {index}");
        assert_eq!(mesure.nominal_len, Fraction::new(3, 4), "mesure {index}");
    }
    assert!(score.measures[0].is_irregular());
    assert!(!score.measures[1].is_irregular());
    assert!(score.measures[2].is_irregular());
    // Hauteur et nombre de notes conservés : le multiplicateur ne change que
    // la durée de la mesure, pas la musique.
    assert_eq!(
        hauteurs_piste(&score, 0),
        vec![vec![72, 74, 76, 79, 72], vec![72, 74], vec![76]]
    );
    assert_eq!(
        hauteurs_piste(&score, 4),
        vec![vec![48, 43, 48, 52, 55], vec![48, 43], vec![48]]
    );
    assert_mesures_coherentes(&score, "levée avec multiplicateur");
}

#[test]
fn partial_multiplicateur_avec_times_et_n_olet() {
    // `\partial 4*3` = trois noires ; les neuf croches étirées par `\times
    // 2/3` totalisent 9/8 × 2/3 = 3/4. La mesure pleine suivante contient un
    // triolet de croches (6 × 1/8 × 2/3 = 1), puis une mesure ordinaire.
    let score = lire(concat!(
        "\\score { \\new Staff { \\fixed c' { ",
        "\\time 4/4 \\partial 4*3 ",
        "\\times 2/3 { c8 c8 c8 c8 c8 c8 c8 c8 c8 } | ",
        "\\tuplet 3/2 { d8 d8 d8 d8 d8 d8 d8 d8 d8 d8 d8 d8 } | ",
        "e4 f g a \\bar \"|.\" } } }"
    ));
    assert_eq!(score.measures.len(), 3);
    assert_eq!(score.measures[0].len, Fraction::new(3, 4));
    assert_eq!(score.measures[1].len, Fraction::ONE);
    assert_eq!(score.measures[2].len, Fraction::ONE);
    assert_mesures_coherentes(&score, "levée avec \\times et n-olet");
}

#[test]
fn partial_sans_multiplicateur_et_multiplicateur_nul_restent_sans_effet() {
    // Cas non concerné : `\partial 4` garde sa longueur d'une noire.
    let simple = lire("\\score { \\new Staff { \\fixed c' { \\partial 4 c4 | c4 d e f } } }");
    assert_eq!(simple.measures[0].len, Fraction::QUARTER);
    assert_eq!(simple.measures[1].len, Fraction::ONE);
    assert_mesures_coherentes(&simple, "levée sans multiplicateur");

    // Cas limite : un facteur nul ne doit pas créer de mesure de longueur
    // nulle (le lecteur retombe sur la métrique, comme pour `\partial 0`).
    let nul = lire("\\score { \\new Staff { \\fixed c' { \\partial 4*0 c4 | c4 d e } } }");
    assert_eq!(nul.measures.len(), 1);
    assert_eq!(nul.measures[0].len, Fraction::ONE);
    assert_mesures_coherentes(&nul, "levée à multiplicateur nul");
}
