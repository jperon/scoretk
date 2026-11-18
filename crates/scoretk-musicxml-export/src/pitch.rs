// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` et
// `engraving/dom/pitchspelling.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Conversions de hauteur et de durée pour l'export MusicXML.
//!
//! Portage des helpers de `exportmusicxml.cpp` : `tpc2stepName`
//! (`engraving/dom/pitchspelling.cpp`), `tpc2alter`, `midipitch2xml`,
//! `tick2xml` et `TDuration::TDuration(const Fraction&)`.

use std::sync::OnceLock;

use scoretk_core::model::{Duration, DurationType, Fraction};

/// Plus petite valeur valide de `Tpc` (`Tpc::TPC_MIN`, fa triple bémol).
///
/// `engraving/dom/pitchspelling.h` : `TPC_INVALID = -9`, puis les quintes
/// commencent à `-8`.
pub(crate) const TPC_MIN: i32 = -8;

/// Nom de degré (`step`) d'un `Tpc`, comme `tpc2stepName`.
///
/// Les `Tpc` valides parcourent la spirale des quintes : fa, do, sol, ré, la,
/// mi, si. Une valeur hors plage (fichier corrompu) retombe sur la même
/// spirale ramenée dans `0..7`.
pub(crate) fn tpc_step_name(tpc: i8) -> char {
    const NAMES: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
    let shift = (i32::from(tpc) - TPC_MIN).rem_euclid(7) as usize;
    NAMES[shift]
}

/// Altération d'un `Tpc` en demi-tons, comme `tpc2alter`.
///
/// La division reproduit celle du C++ (troncature vers zéro) : chaque
/// septuple de quintes ajoute un demi-ton par rapport à la note naturelle.
pub(crate) fn tpc_alter(tpc: i8) -> i32 {
    (i32::from(tpc) - TPC_MIN) / 7 - 3
}

/// Vrai si le `Tpc` est exploitable.
pub(crate) fn tpc_is_valid(tpc: i8) -> bool {
    i32::from(tpc) >= TPC_MIN && i32::from(tpc) <= 40
}

/// Hauteur écrite d'un `Tpc` : degré, altération, octave.
///
/// Reproduit `pitch2xml` : l'octave se déduit de la hauteur MIDI, de la
/// transposition de l'instrument et de l'altération du `Tpc`.
pub(crate) fn written_pitch(pitch: u8, tpc: i8, transpose_chromatic: i32) -> (char, i32, i32) {
    let alter = tpc_alter(tpc);
    let octave = (i32::from(pitch) - transpose_chromatic - alter) / 12 - 1;
    (tpc_step_name(tpc), alter, octave)
}

/// Hauteur déduite de la seule hauteur MIDI, comme `midipitch2xml`.
///
/// Utilisée quand le `Tpc` est invalide : MuseScore ne devrait jamais écrire
/// une telle note, mais le modèle autorise `tpc::INVALID`.
pub(crate) fn midi_pitch(pitch: u8) -> (char, i32, i32) {
    const ALTER: [i32; 12] = [0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0];
    const STEPS: [char; 12] = ['C', 'C', 'D', 'D', 'E', 'F', 'F', 'G', 'G', 'A', 'A', 'B'];
    let index = usize::from(pitch % 12);
    (STEPS[index], ALTER[index], i32::from(pitch) / 12 - 1)
}

/// Figures dans l'ordre décroissant, comme `DurationType` dans MuseScore.
const DURATION_TYPES: [DurationType; 13] = [
    DurationType::Long,
    DurationType::Breve,
    DurationType::Whole,
    DurationType::Half,
    DurationType::Quarter,
    DurationType::Eighth,
    DurationType::Sixteenth,
    DurationType::ThirtySecond,
    DurationType::SixtyFourth,
    DurationType::OneHundredTwentyEighth,
    DurationType::TwoHundredFiftySixth,
    DurationType::FiveHundredTwelfth,
    DurationType::OneThousandTwentyFourth,
];

/// Nom MusicXML d'une figure, comme `TConv::toXml(DurationType)`.
pub(crate) fn duration_type_name(kind: DurationType) -> &'static str {
    match kind {
        DurationType::Long => "long",
        DurationType::Breve => "breve",
        DurationType::Whole => "whole",
        DurationType::Half => "half",
        DurationType::Quarter => "quarter",
        DurationType::Eighth => "eighth",
        DurationType::Sixteenth => "16th",
        DurationType::ThirtySecond => "32nd",
        DurationType::SixtyFourth => "64th",
        DurationType::OneHundredTwentyEighth => "128th",
        DurationType::TwoHundredFiftySixth => "256th",
        DurationType::FiveHundredTwelfth => "512th",
        DurationType::OneThousandTwentyFourth => "1024th",
        DurationType::Measure => "measure",
        DurationType::Zero | DurationType::Invalid => "",
    }
}

/// Table paresseuse des figures précalculées, une seule fois par processus.
///
/// Remplit exactement les 65 candidats parcourus par l'ancien algorithme :
/// figures dans l'ordre de [`DURATION_TYPES`], points de 4 à 0, chaque
/// entrée portant la valeur en fractions de ronde. La recherche dans
/// [`tick_to_xml`] se contente de renvoyer le premier élément dont la
/// fraction est inférieure ou égale à la durée demandée.
fn duration_table() -> &'static [(Fraction, &'static str, u8)] {
    static TABLE: OnceLock<Vec<(Fraction, &'static str, u8)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Vec::with_capacity(DURATION_TYPES.len() * 5);
        for kind in DURATION_TYPES {
            for dots in [4_u8, 3, 2, 1, 0] {
                table.push((
                    Duration::with_dots(kind, dots).ticks(),
                    duration_type_name(kind),
                    dots,
                ));
            }
        }
        table
    })
}

/// Figure et nombre de points correspondant à une durée, comme `tick2xml`.
///
/// `ticks` est exprimé en fractions de ronde. La recherche reproduit
/// `TDuration::TDuration(const Fraction&)` : on part de la figure la plus
/// longue avec quatre points, on réduit les points puis on descend les
/// figures jusqu'à ce que la valeur tienne dans `ticks`. Les valeurs sont
/// précalculées dans [`duration_table`]. Une durée nulle est un silence de
/// mesure entière (`measure`).
pub(crate) fn tick_to_xml(ticks: Fraction) -> Option<(&'static str, u8)> {
    if ticks.is_zero() {
        return Some(("measure", 0));
    }
    if !ticks.is_positive() {
        return None;
    }

    duration_table()
        .iter()
        .find(|&&(value, _, _)| value <= ticks)
        .map(|(_, name, dots)| (*name, *dots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::tpc;

    #[test]
    fn degres_et_alterations() {
        assert_eq!(tpc_step_name(tpc::C), 'C');
        assert_eq!(tpc_step_name(tpc::F), 'F');
        assert_eq!(tpc_step_name(tpc::B), 'B');
        assert_eq!(tpc_step_name(tpc::B_FLAT), 'B');
        assert_eq!(tpc_step_name(tpc::F_SHARP), 'F');
        assert_eq!(tpc_step_name(tpc::C_FLAT), 'C');

        assert_eq!(tpc_alter(tpc::C), 0);
        assert_eq!(tpc_alter(tpc::B_FLAT), -1);
        assert_eq!(tpc_alter(tpc::F_SHARP), 1);
        assert_eq!(tpc_alter(tpc::C_FLAT), -1);
        assert_eq!(tpc_alter(tpc::B_SHARP), 1);
        assert_eq!(tpc_alter(-8), -3);
        assert_eq!(tpc_alter(40), 3);
    }

    #[test]
    fn hauteurs_midi() {
        assert_eq!(written_pitch(60, tpc::C, 0), ('C', 0, 4));
        assert_eq!(written_pitch(70, tpc::B_FLAT, 0), ('B', -1, 4));
        // Clarinette en si bémol : écrit do5, sonne si bémol4 (70).
        assert_eq!(written_pitch(70, tpc::C, -2), ('C', 0, 5));
        assert_eq!(midi_pitch(61), ('C', 1, 4));
    }

    #[test]
    fn figures() {
        assert_eq!(tick_to_xml(Fraction::QUARTER), Some(("quarter", 0)));
        assert_eq!(tick_to_xml(Fraction::new(3, 8)), Some(("quarter", 1)));
        assert_eq!(tick_to_xml(Fraction::new(7, 16)), Some(("quarter", 2)));
        assert_eq!(tick_to_xml(Fraction::ZERO), Some(("measure", 0)));
        assert_eq!(tick_to_xml(Fraction::ONE), Some(("whole", 0)));
        assert_eq!(tick_to_xml(Fraction::new(1, 12)), Some(("16th", 0)));
        assert_eq!(tick_to_xml(Fraction::from_integer(-1)), None);
    }

    /// Ancien algorithme, recopié tel quel avant le passage à la table
    /// précalculée : sert de référence dans le test d'équivalence.
    fn ancien_tick_to_xml(ticks: Fraction) -> Option<(&'static str, u8)> {
        if ticks.is_zero() {
            return Some(("measure", 0));
        }
        if !ticks.is_positive() {
            return None;
        }

        for kind in DURATION_TYPES {
            let mut dots = 4_u8;
            loop {
                if Duration::with_dots(kind, dots).ticks() <= ticks {
                    return Some((duration_type_name(kind), dots));
                }
                if dots == 0 {
                    break;
                }
                dots -= 1;
            }
        }

        None
    }

    #[test]
    fn table_equivalente_a_lancien_algorithme() {
        const DENOMINATEURS: [i64; 14] = [1, 2, 3, 4, 6, 8, 12, 16, 32, 64, 128, 256, 512, 1024];
        for &denominateur in &DENOMINATEURS {
            for numerateur in 1..=200 {
                let ticks = Fraction::new(numerateur, denominateur);
                assert_eq!(
                    tick_to_xml(ticks),
                    ancien_tick_to_xml(ticks),
                    "divergence pour {numerateur}/{denominateur}"
                );
            }
        }
    }
}
