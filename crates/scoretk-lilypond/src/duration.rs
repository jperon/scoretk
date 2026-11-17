// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les formes de durée reprennent celles de `musicxml2ly` (paquet LilyPond,
// GPL-3.0-or-later) : figures `4`, `8`…, points `.`, et multiplicateur `*`.
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

//! Durées : figures, points et multiplicateurs LilyPond.
//!
//! Une durée du modèle est une [`Fraction`] de ronde ; LilyPond attend une
//! figure (`1`, `2`, `4`…) suivie de points, ou un multiplicateur
//! (`1*3/4`). [`length_token`] choisit la figure pointée exacte quand elle
//! existe, et retombe sinon sur `1*num/den`, qui accepte toute fraction
//! positive.

use mscz2xml::model::{Duration, DurationType, Fraction};

/// Commandes LilyPond des figures sans chiffre.
fn base_token(kind: DurationType) -> Option<&'static str> {
    match kind {
        DurationType::Long => Some("\\longa"),
        DurationType::Breve => Some("\\breve"),
        DurationType::Whole | DurationType::Measure => Some("1"),
        DurationType::Half => Some("2"),
        DurationType::Quarter => Some("4"),
        DurationType::Eighth => Some("8"),
        DurationType::Sixteenth => Some("16"),
        DurationType::ThirtySecond => Some("32"),
        DurationType::SixtyFourth => Some("64"),
        DurationType::OneHundredTwentyEighth => Some("128"),
        DurationType::TwoHundredFiftySixth => Some("256"),
        DurationType::FiveHundredTwelfth => Some("512"),
        DurationType::OneThousandTwentyFourth => Some("1024"),
        DurationType::Zero | DurationType::Invalid => None,
    }
}

/// Figure LilyPond d'une durée, points compris.
///
/// Une durée de mesure (`DurationType::Measure`) vaut `1` : l'appelant écrit
/// `R` avec la longueur réelle de la mesure. `Invalid` et `Zero` retombent sur
/// une noire, pour ne jamais produire de jeton vide.
pub(crate) fn duration_token(duration: Duration) -> String {
    let Some(token) = base_token(duration.kind) else {
        return "4".to_owned();
    };
    let mut result = token.to_owned();
    result.push_str(&".".repeat(usize::from(duration.dots)));
    result
}

/// Jeton de durée d'une longueur quelconque, en fractions de ronde.
///
/// Cherche d'abord une figure pointée exacte (`2.`, `4..`), puis retombe sur
/// `1*num/den`, forme qui accepte tout. Une longueur nulle ou négative donne
/// une noire : c'est une valeur de repli, jamais rencontrée dans un fichier
/// lisible.
pub(crate) fn length_token(len: Fraction) -> String {
    if !len.is_positive() {
        return "4".to_owned();
    }
    for kind in [
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
    ] {
        for dots in 0..=3u8 {
            let candidate = Duration::with_dots(kind, dots).ticks();
            if candidate == len {
                return duration_token(Duration::with_dots(kind, dots));
            }
        }
    }
    if len.denominator() == 1 {
        format!("1*{}", len.numerator())
    } else {
        format!("1*{}/{}", len.numerator(), len.denominator())
    }
}

/// Ajoute un multiplicateur `*num/den` quand la durée réelle d'un élément
/// diffère de sa figure notée.
///
/// C'est le cas d'un trémolo à deux notes ou d'une figure étirée : LilyPond
/// doit alors allonger la note pour que la mesure reste juste. Les n-olets en
/// sont exclus : leur durée est déjà mise à l'échelle par `\tuplet`.
pub(crate) fn with_multiplier(token: String, actual: Fraction, notated: Fraction) -> String {
    if notated.is_zero() || actual == notated || !actual.is_positive() {
        return token;
    }
    let ratio = actual / notated;
    format!("{token}*{}/{}", ratio.numerator(), ratio.denominator())
}

/// Longueur d'une mesure en jetons de durée, pour les silences `R`.
pub(crate) fn measure_rest_token(len: Fraction) -> String {
    format!("R{}", length_token(len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_simples() {
        assert_eq!(duration_token(Duration::whole()), "1");
        assert_eq!(duration_token(Duration::quarter()), "4");
        assert_eq!(duration_token(Duration::eighth()), "8");
        assert_eq!(
            duration_token(Duration::with_dots(DurationType::Half, 2)),
            "2.."
        );
        assert_eq!(
            duration_token(Duration::new(DurationType::Breve)),
            "\\breve"
        );
        assert_eq!(duration_token(Duration::measure()), "1");
    }

    #[test]
    fn longueurs_arbitraires() {
        assert_eq!(length_token(Fraction::QUARTER), "4");
        assert_eq!(length_token(Fraction::new(3, 4)), "2.");
        assert_eq!(length_token(Fraction::from_integer(3)), "1*3");
        assert_eq!(length_token(Fraction::new(5, 8)), "1*5/8");
        assert_eq!(length_token(Fraction::ZERO), "4");
        assert_eq!(measure_rest_token(Fraction::new(3, 4)), "R2.");
    }

    #[test]
    fn multiplicateur_des_durees_etirees() {
        assert_eq!(
            with_multiplier("2".to_owned(), Fraction::HALF, Fraction::HALF),
            "2"
        );
        assert_eq!(
            with_multiplier("4".to_owned(), Fraction::new(3, 8), Fraction::QUARTER),
            "4*3/2"
        );
    }
}
