// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les formes de durée suivent la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1) et le comportement de
// `xml2abc.py` et `abc2xml.py` de Willem Vree. Aucune ligne de ces scripts
// n'est copiée : la mise en œuvre est une réécriture Rust.
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

//! Durées ABC : multiplicateurs relatifs à `L:` et figures du modèle.
//!
//! ABC exprime toute durée comme un multiplicateur de l'unité `L:`. Avec
//! `L:1/8`, une croche s'écrit sans multiplicateur, une noire `2`, une
//! blanche `4`, une double croche `/2` et une noire pointée `3`. Les
//! multiplicateurs fractionnaires (`3/2`, `2/3`) couvrent les durées que la
//! notation pointée ne représente pas, en particulier les n-olets.

use mscz2xml::model::{ChordRest, Duration, DurationType, Fraction};

/// Unité d'écriture par défaut (`L:1/8`).
pub(crate) fn unit() -> Fraction {
    Fraction::new(1, 8)
}

/// Multiplicateur ABC d'une durée, relatif à `unit`.
///
/// Une durée nulle ou négative donne une chaîne vide (l'unité), valeur de
/// repli qui ne déplace pas la mesure d'un fichier lisible.
pub(crate) fn multiplier(len: Fraction, unit: Fraction) -> String {
    if !len.is_positive() || !unit.is_positive() {
        return String::new();
    }
    let ratio = len / unit;
    if ratio == Fraction::ONE {
        return String::new();
    }
    if ratio.denominator() == 1 {
        return ratio.numerator().to_string();
    }
    if ratio.numerator() == 1 {
        return format!("/{}", ratio.denominator());
    }
    format!("{}/{}", ratio.numerator(), ratio.denominator())
}

/// Longueur notée d'un accord ou silence.
///
/// Un n-olet garde sa figure écrite (c'est `(p` qui met l'échelle) ; les
/// autres éléments utilisent leur durée réelle quand elle diffère de la
/// figure (trémolo, figure étirée).
pub(crate) fn chord_rest_length(chord_rest: &ChordRest) -> Fraction {
    let notated = chord_rest.duration.ticks();
    if chord_rest.tuplet.is_some() {
        return notated;
    }
    chord_rest.ticks.unwrap_or(notated)
}

/// Figure du modèle correspondant exactement à une longueur, si elle existe.
///
/// `Some((duration, true))` pour une figure exacte ; sinon la figure la plus
/// proche et `false`, l'appelant devant conserver la durée réelle dans
/// [`ChordRest::ticks`].
pub(crate) fn figure_for(len: Fraction) -> (Duration, bool) {
    let mut best = Duration::quarter();
    for kind in [
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
    ] {
        for dots in 0..=3u8 {
            let candidate = Duration::with_dots(kind, dots);
            if candidate.ticks() == len {
                return (candidate, true);
            }
            if candidate.ticks() <= len {
                best = candidate;
            }
        }
    }
    (best, false)
}

/// Multiplie une figure par `factor` quand c'est exact (rythme brisé).
///
/// `factor` vaut trois demis pour un point, sept quarts pour deux points. La
/// multiplication augmente les points quand c'est possible ; sinon elle
/// renvoie `None`.
pub(crate) fn dotted(duration: Duration, factor: Fraction) -> Option<Duration> {
    if factor == Fraction::new(3, 2) {
        return Some(Duration::with_dots(duration.kind, duration.dots + 1));
    }
    if factor == Fraction::new(7, 4) {
        return Some(Duration::with_dots(duration.kind, duration.dots + 2));
    }
    None
}

/// Divise une figure par `2^n` quand c'est possible (rythme brisé).
pub(crate) fn halved(duration: Duration, levels: u32) -> Option<Duration> {
    const KINDS: [DurationType; 13] = [
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
    let index = KINDS.iter().position(|kind| *kind == duration.kind)?;
    let target = index + levels as usize;
    KINDS
        .get(target)
        .map(|kind| Duration::with_dots(*kind, duration.dots))
}

/// Analyse un multiplicateur ABC (`""`, `2`, `/2`, `3/2`, `//`, `/`).
///
/// Renvoie la longueur en fraction de ronde, ou `None` si le texte est
/// invalide.
pub(crate) fn parse_multiplier(text: &str, unit: Fraction) -> Option<Fraction> {
    if text.is_empty() {
        return Some(unit);
    }
    if let Some(rest) = text.strip_prefix('/') {
        let extra = rest
            .chars()
            .take_while(|character| *character == '/')
            .count();
        let digits = &rest[extra..];
        let denominator = if digits.is_empty() {
            2
        } else {
            digits.parse::<i64>().ok()?
        };
        if denominator == 0 {
            return None;
        }
        return Some(unit / Fraction::from_integer(denominator * (1 << extra)));
    }
    if let Some((numerator, denominator)) = text.split_once('/') {
        let numerator: i64 = numerator.parse().ok()?;
        let denominator: i64 = if denominator.is_empty() {
            2
        } else {
            denominator.parse().ok()?
        };
        if numerator == 0 || denominator == 0 {
            return None;
        }
        return Some(unit * Fraction::new(numerator, denominator));
    }
    let factor: i64 = text.parse().ok()?;
    if factor == 0 {
        return None;
    }
    Some(unit * Fraction::from_integer(factor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplicateurs() {
        assert_eq!(multiplier(Fraction::new(1, 8), unit()), "");
        assert_eq!(multiplier(Fraction::new(1, 4), unit()), "2");
        assert_eq!(multiplier(Fraction::new(1, 16), unit()), "/2");
        assert_eq!(multiplier(Fraction::new(3, 16), unit()), "3/2");
        assert_eq!(multiplier(Fraction::new(1, 12), unit()), "2/3");
        assert_eq!(multiplier(Fraction::from_integer(1), unit()), "8");
    }

    #[test]
    fn analyse_des_multiplicateurs() {
        let u = unit();
        assert_eq!(parse_multiplier("", u), Some(u));
        assert_eq!(parse_multiplier("2", u), Some(Fraction::new(1, 4)));
        assert_eq!(parse_multiplier("/2", u), Some(Fraction::new(1, 16)));
        assert_eq!(parse_multiplier("/", u), Some(Fraction::new(1, 16)));
        assert_eq!(parse_multiplier("//", u), Some(Fraction::new(1, 32)));
        assert_eq!(parse_multiplier("3/2", u), Some(Fraction::new(3, 16)));
        assert_eq!(parse_multiplier("2/3", u), Some(Fraction::new(1, 12)));
        assert_eq!(parse_multiplier("0", u), None);
        assert_eq!(parse_multiplier("x", u), None);
    }

    #[test]
    fn figures() {
        assert_eq!(figure_for(Fraction::new(1, 8)), (Duration::eighth(), true));
        assert_eq!(
            figure_for(Fraction::new(3, 16)),
            (Duration::with_dots(DurationType::Eighth, 1), true)
        );
        assert_eq!(
            figure_for(Fraction::new(1, 12)).1,
            false,
            "un triolet ne s'écrit pas avec une figure pointée"
        );
        assert_eq!(
            dotted(Duration::eighth(), Fraction::new(3, 2)),
            Some(Duration::with_dots(DurationType::Eighth, 1))
        );
        assert_eq!(
            halved(Duration::eighth(), 1),
            Some(Duration::new(DurationType::Sixteenth))
        );
    }
}
