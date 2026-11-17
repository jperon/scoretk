// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
//
// Portage de `engraving/types/fraction.h` de MuseScore Studio, Copyright (C)
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

//! Fractions rationnelles exactes.
//!
//! Portage de `Fraction` de MuseScore (`engraving/types/fraction.h`). Le type
//! reste toujours sous forme réduite, avec un dénominateur strictement positif,
//! de sorte que l'égalité, le hachage et l'ordre sont canoniques.
//!
//! Dans MuseScore, une position (`tick`) est une `Fraction` exprimée en
//! **rondes** : une mesure 4/4 vaut `1/1`, une noire vaut `1/4`. C'est aussi
//! l'unité de [`Tick`] et de [`super::Duration::ticks`]. La conversion vers les
//! divisions MusicXML (qui comptent, elles, les unités par **noire**) se fait
//! avec [`Fraction::ticks`] en passant `4 * divisions`.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Fraction rationnelle exacte, toujours réduite.
///
/// Correspond à `Fraction` dans `engraving/types/fraction.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fraction {
    numerator: i64,
    denominator: i64,
}

/// Position ou durée exprimée en fraction de ronde, comme `Fraction` dans
/// MuseScore.
pub type Tick = Fraction;

impl Fraction {
    /// Zéro (`0/1`).
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// Un (`1/1`), c'est-à-dire une ronde.
    pub const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    /// Un demi (`1/2`).
    pub const HALF: Self = Self {
        numerator: 1,
        denominator: 2,
    };

    /// Un quart (`1/4`), c'est-à-dire une noire.
    pub const QUARTER: Self = Self {
        numerator: 1,
        denominator: 4,
    };

    /// Construit une fraction réduite.
    ///
    /// # Panics
    ///
    /// Panique si `denominator` vaut zéro ou si la fraction normalisée ne tient
    /// pas dans `i64`. Pour une entrée non fiable, préférer
    /// [`Fraction::try_new`].
    pub fn new(numerator: i64, denominator: i64) -> Self {
        Self::try_new(numerator, denominator)
            .expect("fraction invalide : dénominateur nul ou dépassement de i64")
    }

    /// Construit une fraction réduite, ou renvoie `None` si elle est invalide.
    pub fn try_new(numerator: i64, denominator: i64) -> Option<Self> {
        normalize(i128::from(numerator), i128::from(denominator))
    }

    /// Construit une fraction entière.
    pub const fn from_integer(value: i64) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    /// Convertit un nombre d'unités en fraction.
    ///
    /// `units_per_whole` est le nombre d'unités par ronde. Pour des divisions
    /// MusicXML (unités par noire), passer `4 * divisions`.
    ///
    /// # Panics
    ///
    /// Panique si `units_per_whole` vaut zéro.
    pub fn from_ticks(ticks: i64, units_per_whole: u32) -> Self {
        Self::try_from_ticks(ticks, units_per_whole)
            .expect("conversion ticks -> Fraction impossible : unités par ronde nulles")
    }

    /// Variante non paniquante de [`Fraction::from_ticks`].
    pub fn try_from_ticks(ticks: i64, units_per_whole: u32) -> Option<Self> {
        if units_per_whole == 0 {
            return None;
        }
        Self::try_new(ticks, i64::from(units_per_whole))
    }

    /// Numérateur (signé).
    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    /// Dénominateur (toujours strictement positif).
    pub const fn denominator(self) -> i64 {
        self.denominator
    }

    /// Convertit la fraction en nombre entier d'unités.
    ///
    /// La division est entière et tronque vers zéro, comme la division C++ de
    /// MuseScore. `units_per_whole` est le nombre d'unités par ronde : pour des
    /// divisions MusicXML, passer `4 * divisions`.
    ///
    /// # Panics
    ///
    /// Panique si `units_per_whole` vaut zéro ou si le résultat ne tient pas
    /// dans `i64`.
    pub fn ticks(self, units_per_whole: u32) -> i64 {
        assert!(
            units_per_whole > 0,
            "conversion Fraction -> ticks impossible : unités par ronde nulles"
        );
        let value =
            i128::from(self.numerator) * i128::from(units_per_whole) / i128::from(self.denominator);
        i64::try_from(value).expect("conversion Fraction -> ticks : dépassement de i64")
    }

    /// Valeur approchée en virgule flottante.
    pub fn as_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    /// Vrai si la fraction est nulle.
    pub const fn is_zero(self) -> bool {
        self.numerator == 0
    }

    /// Vrai si la fraction est strictement positive.
    pub const fn is_positive(self) -> bool {
        self.numerator > 0
    }

    /// Vrai si la fraction est strictement négative.
    pub const fn is_negative(self) -> bool {
        self.numerator < 0
    }

    /// Valeur absolue.
    pub fn abs(self) -> Self {
        from_i128(
            i128::from(self.numerator).abs(),
            i128::from(self.denominator),
        )
    }

    /// Inverse.
    ///
    /// # Panics
    ///
    /// Panique si la fraction est nulle.
    pub fn recip(self) -> Self {
        Self::new(self.denominator, self.numerator)
    }

    /// Élève la fraction à une puissance entière positive ou nulle.
    pub fn pow(self, exponent: u32) -> Self {
        let mut result = Self::ONE;
        for _ in 0..exponent {
            result *= self;
        }
        result
    }
}

impl Default for Fraction {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = i128::from(self.numerator) * i128::from(other.denominator);
        let right = i128::from(other.numerator) * i128::from(self.denominator);
        left.cmp(&right)
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for Fraction {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let numerator = i128::from(self.numerator) * i128::from(rhs.denominator)
            + i128::from(rhs.numerator) * i128::from(self.denominator);
        let denominator = i128::from(self.denominator) * i128::from(rhs.denominator);
        from_i128(numerator, denominator)
    }
}

impl Sub for Fraction {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl Mul for Fraction {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        from_i128(
            i128::from(self.numerator) * i128::from(rhs.numerator),
            i128::from(self.denominator) * i128::from(rhs.denominator),
        )
    }
}

impl Div for Fraction {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        from_i128(
            i128::from(self.numerator) * i128::from(rhs.denominator),
            i128::from(self.denominator) * i128::from(rhs.numerator),
        )
    }
}

impl Neg for Fraction {
    type Output = Self;

    fn neg(self) -> Self {
        from_i128(-i128::from(self.numerator), i128::from(self.denominator))
    }
}

impl AddAssign for Fraction {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fraction {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Fraction {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for Fraction {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl From<i64> for Fraction {
    fn from(value: i64) -> Self {
        Self::from_integer(value)
    }
}

impl From<i32> for Fraction {
    fn from(value: i32) -> Self {
        Self::from_integer(i64::from(value))
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

/// Normalise `numerator / denominator`, ou renvoie `None` si le dénominateur
/// est nul ou si le résultat ne tient pas dans `i64`.
fn normalize(numerator: i128, denominator: i128) -> Option<Fraction> {
    if denominator == 0 {
        return None;
    }

    let numerator = if denominator < 0 {
        numerator.checked_neg()?
    } else {
        numerator
    };
    let denominator = denominator.abs();
    let divisor = gcd(numerator.unsigned_abs(), denominator as u128) as i128;

    Some(Fraction {
        numerator: i64::try_from(numerator / divisor).ok()?,
        denominator: i64::try_from(denominator / divisor).ok()?,
    })
}

/// Construit une fraction à partir d'un calcul interne, en paniquant sur
/// dépassement (les opérandes d'origine sont des `i64`).
fn from_i128(numerator: i128, denominator: i128) -> Fraction {
    normalize(numerator, denominator)
        .expect("dépassement de i64 pendant une opération sur Fraction")
}

/// PGCD de deux entiers non signés.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduction_et_signe() {
        assert_eq!(Fraction::new(6, 4), Fraction::new(3, 2));
        assert_eq!(Fraction::new(-6, 4), Fraction::new(3, -2));
        assert_eq!(Fraction::new(0, -7), Fraction::ZERO);
        assert_eq!(Fraction::new(4, 4), Fraction::ONE);
        assert_eq!(Fraction::new(6, 4).numerator(), 3);
        assert_eq!(Fraction::new(6, 4).denominator(), 2);
    }

    #[test]
    fn addition_et_soustraction() {
        let half = Fraction::new(1, 2);
        let third = Fraction::new(1, 3);
        assert_eq!(half + third, Fraction::new(5, 6));
        assert_eq!(half - third, Fraction::new(1, 6));
        assert_eq!(half + half, Fraction::ONE);
        assert_eq!(-Fraction::new(1, 3), Fraction::new(-1, 3));
    }

    #[test]
    fn multiplication_division_puissance() {
        assert_eq!(Fraction::new(2, 3) * Fraction::new(3, 4), Fraction::HALF);
        assert_eq!(
            Fraction::new(2, 3) / Fraction::new(4, 9),
            Fraction::new(3, 2)
        );
        assert_eq!(Fraction::new(2, 3).recip(), Fraction::new(3, 2));
        assert_eq!(Fraction::HALF.pow(3), Fraction::new(1, 8));
        assert_eq!(Fraction::new(4, 9).pow(0), Fraction::ONE);
    }

    #[test]
    fn comparaison() {
        assert!(Fraction::new(2, 3) < Fraction::new(3, 4));
        assert!(Fraction::new(-1, 2) < Fraction::ZERO);
        assert_eq!(
            Fraction::new(10, 15).cmp(&Fraction::new(2, 3)),
            Ordering::Equal
        );
        assert_eq!(
            [Fraction::new(3, 4), Fraction::HALF, Fraction::ONE]
                .into_iter()
                .min(),
            Some(Fraction::HALF)
        );
    }

    #[test]
    fn conversion_ticks_et_divisions() {
        // 480 unités par ronde : une blanche pointée = 3/4 de ronde = 360.
        assert_eq!(Fraction::new(3, 4).ticks(480), 360);
        assert_eq!(Fraction::from_ticks(360, 480), Fraction::new(3, 4));
        assert_eq!(Fraction::new(3, 2).ticks(480), 720);
        assert_eq!(Fraction::from_ticks(720, 480), Fraction::new(3, 2));
        // Troncature vers zéro, comme la division C++.
        assert_eq!(Fraction::new(1, 3).ticks(480), 160);
        assert_eq!(Fraction::new(-1, 3).ticks(480), -160);
        assert_eq!(Fraction::new(1, 7).ticks(480), 68);
    }

    #[test]
    fn division_par_zero_refusee() {
        assert_eq!(Fraction::try_new(1, 0), None);
        assert_eq!(Fraction::try_from_ticks(1, 0), None);
    }

    #[test]
    fn affichage() {
        assert_eq!(Fraction::new(3, 2).to_string(), "3/2");
        assert_eq!(Fraction::ZERO.to_string(), "0/1");
    }

    #[test]
    fn multiplication_par_un_negatif_ne_casse_pas_la_normalisation() {
        assert_eq!(
            Fraction::new(3, 2) * Fraction::new(-2, 3),
            Fraction::new(-1, 1)
        );
        assert_eq!((-Fraction::new(3, 2)).ticks(480), -720);
    }
}
