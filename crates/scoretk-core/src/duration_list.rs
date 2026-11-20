// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/durationtype.cpp::toDurationList` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Décompose une durée en figures notables, la plus grande d'abord.
//!
//! Port unique de `engraving/dom/durationtype.cpp::toDurationList`
//! (`std::vector<TDuration> toDurationList(Fraction l, bool useDots,
//! int maxDots = 4, bool printRestRemains = true)`). Le paramètre
//! `print_rest_remains` est conservé pour épouser la signature du C++, mais
//! cette bibliothèque sans I/O ne journalise rien : la durée résiduelle est
//! simplement ignorée, comme quand le drapeau est faux côté MuseScore.
//!
//! Les types sont essayés du plus long au plus court, et pour chacun le plus
//! grand nombre de points (jusqu'à `max_dots`) qui tient dans la durée : c'est
//! l'ordre du constructeur `TDuration(l, true, maxDots)` du C++. `use_dots =
//! false` équivaut à `maxDots = 0`, ce qui donne les figures sans point utilisées
//! par le remplissage des trous de voix.

use crate::model::{Duration, DurationType, Fraction};

/// Décompose `length` (en fractions de ronde) en figures exactes.
///
/// `max_dots` suit la valeur par défaut du C++ (4) chez les appelants qui
/// notent de la musique ; `Meter` de l'import MIDI passe 1.
pub fn to_duration_list(
    mut length: Fraction,
    use_dots: bool,
    max_dots: u8,
    _print_rest_remains: bool,
) -> Vec<Duration> {
    let max_dots = if use_dots { max_dots } else { 0 };
    let mut result = Vec::new();

    // Figures de la plus longue à la plus courte, comme `DurationType`.
    const TYPES: [DurationType; 14] = [
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
        DurationType::Zero,
    ];

    while length.is_positive() {
        let mut found = None;
        for kind in TYPES {
            if matches!(kind, DurationType::Zero) {
                break;
            }
            let mut dots = i32::from(max_dots);
            while dots >= 0 {
                let duration = Duration::with_dots(kind, dots as u8);
                let fraction = duration.ticks();
                if fraction <= length {
                    found = Some(duration);
                    break;
                }
                dots -= 1;
            }
            if found.is_some() {
                break;
            }
        }
        match found {
            Some(duration) => {
                length -= duration.ticks();
                result.push(duration);
            }
            None => break,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_avec_et_sans_points() {
        assert_eq!(
            to_duration_list(Fraction::new(3, 8), true, 4, false),
            vec![Duration::with_dots(DurationType::Quarter, 1)]
        );
        assert_eq!(
            to_duration_list(Fraction::new(5, 16), true, 4, false),
            vec![
                Duration::new(DurationType::Quarter),
                Duration::new(DurationType::Sixteenth),
            ]
        );
        assert_eq!(
            to_duration_list(Fraction::new(7, 16), true, 4, false),
            vec![Duration::with_dots(DurationType::Quarter, 2)]
        );
        assert_eq!(
            to_duration_list(Fraction::new(7, 16), false, 4, false),
            vec![
                Duration::new(DurationType::Quarter),
                Duration::new(DurationType::Eighth),
                Duration::new(DurationType::Sixteenth),
            ]
        );
    }

    #[test]
    fn long_breve_et_bornes() {
        assert_eq!(
            to_duration_list(Fraction::new(6, 1), true, 4, false),
            vec![Duration::with_dots(DurationType::Long, 1)]
        );
        assert!(to_duration_list(Fraction::ZERO, true, 4, false).is_empty());
        assert!(to_duration_list(Fraction::new(-1, 4), true, 4, false).is_empty());
    }

    #[test]
    fn somme_exacte() {
        for numerator in 1..32 {
            let length = Fraction::new(numerator, 32);
            let somme = to_duration_list(length, true, 4, false)
                .iter()
                .fold(Fraction::ZERO, |acc, duration| acc + duration.ticks());
            assert_eq!(somme, length);
        }
    }
}
