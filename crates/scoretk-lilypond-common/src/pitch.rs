// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les tables de correspondance reprennent le comportement de `musicxml2ly`
// (paquet LilyPond, GPL-3.0-or-later) et de `engraving/dom/pitchspelling.cpp`
// de MuseScore Studio ; le code ci-dessous est une réécriture en Rust, sans
// ligne copiée.
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

//! Spirale des quintes (`tpc`), partagée par la lecture et l'écriture.
//!
//! Le modèle porte deux informations de hauteur : [`Note::pitch`], la hauteur
//! MIDI **réelle** (celle du fichier `.mscx`), et [`Note::tpc`], la classe de
//! hauteur tonale qui fixe l'orthographe (do dièse plutôt que ré bémol).
//! L'écriture en déduit le degré et l'altération à écrire ; la lecture fait
//! l'inverse pour remplir le modèle. Les deux sens partagent donc la même
//! table.
//!
//! [`Note::pitch`]: scoretk_core::model::Note::pitch
//! [`Note::tpc`]: scoretk_core::model::Note::tpc
//!
//! # Orthographe
//!
//! `tpc` parcourt la spirale des quintes : `tpc - TPC_MIN` modulo 7 donne le
//! degré (fa, do, sol, ré, la, mi, si) et la division entière par 7 donne
//! l'altération, comme `tpc2stepName` et `tpc2alter` de MuseScore
//! (`engraving/dom/pitchspelling.cpp`).

use scoretk_core::model::tpc;

/// Plus petite valeur de `tpc` valide (`Tpc::TPC_MIN`, fa triple bémol).
pub const TPC_MIN: i32 = -8;

/// Degrés de la spirale des quintes.
pub const STEP_NAMES: [char; 7] = ['f', 'c', 'g', 'd', 'a', 'e', 'b'];

/// Classe de hauteur tonale d'un degré naturel, indexée par degré
/// (0 = do … 6 = si).
const NATURAL_TPC: [i8; 7] = [tpc::C, tpc::D, tpc::E, tpc::F, tpc::G, tpc::A, tpc::B];

/// Vrai si le `tpc` est exploitable (`tpc::INVALID` sinon).
pub fn valid(tpc_value: i8) -> bool {
    (TPC_MIN..=40).contains(&(i32::from(tpc_value)))
}

/// Degré d'un `tpc`, comme `tpc2stepName`.
pub fn step(tpc_value: i8) -> char {
    let index = (i32::from(tpc_value) - TPC_MIN).rem_euclid(7) as usize;
    STEP_NAMES[index]
}

/// Altération en demi-tons d'un `tpc`, comme `tpc2alter`.
pub fn alter(tpc_value: i8) -> i32 {
    (i32::from(tpc_value) - TPC_MIN) / 7 - 3
}

/// Classe de hauteur tonale (`tpc`) d'un degré et d'une altération.
pub fn tpc_of(step: u8, alter: i8) -> i8 {
    NATURAL_TPC[usize::from(step)] + 7 * alter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degres_et_altered() {
        assert_eq!(step(tpc::C), 'c');
        assert_eq!(step(tpc::F), 'f');
        assert_eq!(step(tpc::B), 'b');
        assert_eq!(alter(tpc::C), 0);
        assert_eq!(alter(tpc::C_SHARP), 1);
        assert_eq!(alter(tpc::B_FLAT), -1);
        assert_eq!(alter(tpc::F_SHARP), 1);
        assert_eq!(alter(tpc::C_FLAT), -1);
        assert!(valid(tpc::C));
        assert!(!valid(tpc::INVALID));
    }

    #[test]
    fn spirale_aller_retour() {
        for step_value in 0u8..7 {
            for alter_value in -2i8..=2 {
                let tpc_value = tpc_of(step_value, alter_value);
                let degree = match step(tpc_value) {
                    'c' => 0,
                    'd' => 1,
                    'e' => 2,
                    'f' => 3,
                    'g' => 4,
                    'a' => 5,
                    _ => 6,
                };
                assert_eq!(degree, step_value);
                assert_eq!(alter(tpc_value), i32::from(alter_value));
            }
        }
    }
}
