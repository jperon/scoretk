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

//! Contrôle de cohérence du modèle : dans chaque mesure, les durées des
//! accords et silences d'une voix doivent totaliser la longueur réelle de la
//! mesure. C'est l'invariant que MuseScore vérifie en rouvrant un `.mscz` et
//! qui produit « Incomplete measure … Found: 5/8. Expected: 1/8 » quand il est
//! violé (cf. `docs/agents/mscx-write.md`, § « Second pass — the 86 files
//! still unopened »).

use scoretk_core::model::{Fraction, Score};

/// Écart entre la longueur d'une mesure et la somme des durées d'une voix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncoherenceMesure {
    /// Index de la mesure (0-basé).
    pub mesure: usize,
    /// Piste de la voix concernée.
    pub piste: u32,
    /// Longueur réelle de la mesure (`Measure::len`).
    pub attendu: Fraction,
    /// Somme des durées effectives des accords et silences de la voix.
    pub trouve: Fraction,
}

/// Durée effective d'un accord ou silence, n-olets compris.
///
/// `ChordRest::ticks` est la durée réelle que le lecteur a explicitée
/// (multiplicateur `*N`), mise à l'échelle comprise ; sinon on applique les
/// ratios des n-olets empilés, comme `DurationElement::actualTicks()` de
/// MuseScore.
fn duree_effective(
    score: &Score,
    mesure_len: Fraction,
    chord_rest: &scoretk_core::model::ChordRest,
) -> Fraction {
    if let Some(ticks) = chord_rest.ticks {
        return ticks;
    }
    let mut duree = chord_rest.ticks_in_measure(mesure_len);
    let mut noelet = chord_rest.tuplet;
    while let Some(id) = noelet {
        let tuplet = score.tuplet(id);
        if tuplet.ratio.numerator() != 0 {
            duree *= Fraction::new(tuplet.ratio.denominator(), tuplet.ratio.numerator());
        }
        noelet = tuplet.parent;
    }
    duree
}

/// Écarts de cohérence de toutes les mesures, toutes voix confondues.
pub fn incoherences_mesures(score: &Score) -> Vec<IncoherenceMesure> {
    let mut ecarts = Vec::new();
    for (index, mesure) in score.measures.iter().enumerate() {
        let mut pistes: Vec<_> = mesure
            .segments
            .iter()
            .flat_map(|segment| segment.iter_elements())
            .filter(|(_, item)| item.as_chord_rest().is_some())
            .map(|(track, _)| track)
            .collect();
        pistes.sort_unstable();
        pistes.dedup();
        for piste in pistes {
            let somme = mesure
                .segments
                .iter()
                .filter_map(|segment| segment.chord_rest(piste))
                .fold(Fraction::ZERO, |total, chord_rest| {
                    total + duree_effective(score, mesure.len, chord_rest)
                });
            if somme != mesure.len {
                ecarts.push(IncoherenceMesure {
                    mesure: index,
                    piste: piste.raw(),
                    attendu: mesure.len,
                    trouve: somme,
                });
            }
        }
    }
    ecarts
}

/// Échoue si une voix d'une mesure ne couvre pas exactement cette mesure.
pub fn assert_mesures_coherentes(score: &Score, contexte: &str) {
    let ecarts = incoherences_mesures(score);
    assert!(ecarts.is_empty(), "{contexte} : {ecarts:?}");
}
