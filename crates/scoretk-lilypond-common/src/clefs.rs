// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les tables de correspondance reprennent le comportement de `musicxml2ly`
// (paquet LilyPond, GPL-3.0-or-later) ; le code ci-dessous est une réécriture
// en Rust, sans ligne copiée.
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

//! Clefs : correspondance entre [`ClefType`] et nom LilyPond.
//!
//! Une seule table sert les deux sens : l'écriture cherche le nom d'une clef du
//! modèle, la lecture retrouve la clef d'un nom de `\clef`. Les noms octaviés
//! s'écrivent `treble_8`, `treble^8`, `bass_15`, `bass^15`, comme les commandes
//! prédéfinies du paquet LilyPond ; les clefs d'ut suivent la table de
//! `musicxml2ly` (`soprano`, `mezzosoprano`, `alto`, `tenor`, `baritone`).

use scoretk_core::model::ClefType;

/// Correspondance [`ClefType`] → nom canonique LilyPond.
///
/// Plusieurs variantes du modèle partagent un même nom LilyPond (les quatre
/// tablatures, `percussion` pour deux clefs) : c'est sans perte, LilyPond ne
/// les distinguant pas non plus.
pub const CLEFS: &[(ClefType, &str)] = &[
    (ClefType::G, "treble"),
    (ClefType::G8Vb, "treble_8"),
    (ClefType::G8Va, "treble^8"),
    (ClefType::G15Mb, "treble_15"),
    (ClefType::G15Ma, "treble^15"),
    (ClefType::G1, "french"),
    (ClefType::C1, "soprano"),
    (ClefType::C2, "mezzosoprano"),
    (ClefType::C3, "alto"),
    (ClefType::C4, "tenor"),
    (ClefType::C5, "baritone"),
    (ClefType::F, "bass"),
    (ClefType::F8Vb, "bass_8"),
    (ClefType::F8Va, "bass^8"),
    (ClefType::F15Mb, "bass_15"),
    (ClefType::F15Ma, "bass^15"),
    (ClefType::FBaritone, "varbaritone"),
    (ClefType::FSubbass, "subbass"),
    (ClefType::Perc, "percussion"),
    (ClefType::Perc2, "percussion"),
    (ClefType::Tab, "tab"),
    (ClefType::Tab4, "tab"),
    (ClefType::TabSerif, "tab"),
    (ClefType::Tab4Serif, "tab"),
];

/// Nom LilyPond d'une clef du modèle, sans guillemets.
///
/// `None` pour une clef invalide. L'écriture décide ensuite d'entourer de
/// guillemets les noms qui ne sont pas de simples commandes (`treble_8`,
/// `tab`).
pub fn name(clef: ClefType) -> Option<&'static str> {
    CLEFS
        .iter()
        .find(|(candidate, _)| *candidate == clef)
        .map(|(_, name)| *name)
}

/// Clef correspondant à un nom de LilyPond.
///
/// Accepte les noms canoniques de [`CLEFS`], leurs alias (`G`, `C`, `F`,
/// `C1`…`C5`, `violin`) et les marques d'octave (`treble^8`, `bass_15`…).
pub fn parse(name: &str) -> Option<ClefType> {
    if let Some((clef, _)) = CLEFS.iter().find(|(_, candidate)| *candidate == name) {
        return Some(*clef);
    }
    let (base, octave) = match name.split_once('^') {
        Some((base, shift)) => (base, octave_shift(shift)?),
        None => match name.split_once('_') {
            Some((base, shift)) => (base, -octave_shift(shift)?),
            None => (name, 0),
        },
    };
    let clef = match (base, octave) {
        ("treble" | "violin" | "G", 0) => ClefType::G,
        ("treble", -1) | ("G", -1) => ClefType::G8Vb,
        ("treble", 1) | ("G", 1) => ClefType::G8Va,
        ("treble", -2) | ("G", -2) => ClefType::G15Mb,
        ("treble", 2) | ("G", 2) => ClefType::G15Ma,
        ("french", 0) => ClefType::G1,
        ("soprano", 0) | ("C1", 0) => ClefType::C1,
        ("mezzosoprano", 0) | ("C2", 0) => ClefType::C2,
        ("alto" | "C", 0) => ClefType::C3,
        ("tenor", 0) | ("C4", 0) => ClefType::C4,
        ("baritone", 0) | ("C5", 0) => ClefType::C5,
        ("bass" | "F", 0) => ClefType::F,
        ("bass", -1) | ("F", -1) => ClefType::F8Vb,
        ("bass", 1) | ("F", 1) => ClefType::F8Va,
        ("bass", -2) | ("F", -2) => ClefType::F15Mb,
        ("bass", 2) | ("F", 2) => ClefType::F15Ma,
        ("varbaritone", 0) => ClefType::FBaritone,
        ("subbass", 0) => ClefType::FSubbass,
        ("percussion", 0) => ClefType::Perc,
        ("tab", 0) => ClefType::Tab,
        _ => return None,
    };
    Some(clef)
}

/// Nombre d'octaves d'une marque de clef (`8` → 1, `15` → 2).
fn octave_shift(shift: &str) -> Option<i8> {
    match shift.parse::<i8>().ok()? {
        8 => Some(1),
        15 => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_canoniques() {
        assert_eq!(name(ClefType::G), Some("treble"));
        assert_eq!(name(ClefType::F8Vb), Some("bass_8"));
        assert_eq!(name(ClefType::Invalid), None);
        for (clef, lilypond) in CLEFS {
            assert_eq!(name(*clef), Some(*lilypond));
        }
    }

    #[test]
    fn lecture_des_noms() {
        assert_eq!(parse("treble"), Some(ClefType::G));
        assert_eq!(parse("treble_8"), Some(ClefType::G8Vb));
        assert_eq!(parse("bass^8"), Some(ClefType::F8Va));
        assert_eq!(parse("alto"), Some(ClefType::C3));
        assert_eq!(parse("C4"), Some(ClefType::C4));
        assert_eq!(parse("percussion"), Some(ClefType::Perc));
        assert_eq!(parse("tab"), Some(ClefType::Tab));
        assert_eq!(parse("inconnue"), None);
    }
}
