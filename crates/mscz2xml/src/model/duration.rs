// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `engraving/dom/durationtype.h` et `engraving/types/types.h` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Durées musicales et figures de note.
//!
//! Portage de `TDuration` et de l'énumération `DurationType`
//! (`engraving/dom/durationtype.h`, `engraving/types/types.h`). Comme dans
//! MuseScore, une durée s'exprime en fractions de **ronde** : une noire vaut
//! `1/4`, une blanche `1/2`, une ronde `1/1`.

use super::fraction::Fraction;
use super::note::NoteHeadType;

/// Figure de durée d'une note ou d'un silence.
///
/// Correspond à `DurationType` dans `engraving/types/types.h`, manipulé par
/// `TDuration` dans `engraving/dom/durationtype.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DurationType {
    /// Longue (`V_LONG`), quatre rondes.
    Long,
    /// Brève (`V_BREVE`), deux rondes.
    Breve,
    /// Ronde (`V_WHOLE`).
    Whole,
    /// Blanche (`V_HALF`).
    Half,
    /// Noire (`V_QUARTER`).
    Quarter,
    /// Croche (`V_EIGHTH`).
    Eighth,
    /// Double croche (`V_16TH`).
    Sixteenth,
    /// Triple croche (`V_32ND`).
    ThirtySecond,
    /// Quadruple croche (`V_64TH`).
    SixtyFourth,
    /// Cent-vingt-huitième (`V_128TH`).
    OneHundredTwentyEighth,
    /// Deux-cent-cinquante-sixième (`V_256TH`).
    TwoHundredFiftySixth,
    /// Cinq-cent-douzième (`V_512TH`).
    FiveHundredTwelfth,
    /// Mille-vingt-quatrième (`V_1024TH`).
    OneThousandTwentyFourth,
    /// Durée nulle (`V_ZERO`).
    Zero,
    /// Silence de mesure entière (`V_MEASURE`).
    Measure,
    /// Durée invalide (`V_INVALID`).
    Invalid,
}

impl DurationType {
    /// Durée en fractions de ronde (ronde = `1/1`).
    ///
    /// `Measure` et `Zero` valent zéro ; `Invalid` vaut `-1`, comme
    /// `TDuration::ticks()` dans `engraving/dom/durationtype.cpp`.
    pub fn ticks(self) -> Fraction {
        match self {
            Self::Long => Fraction::from_integer(4),
            Self::Breve => Fraction::from_integer(2),
            Self::Whole => Fraction::from_integer(1),
            Self::Half => Fraction::new(1, 2),
            Self::Quarter => Fraction::new(1, 4),
            Self::Eighth => Fraction::new(1, 8),
            Self::Sixteenth => Fraction::new(1, 16),
            Self::ThirtySecond => Fraction::new(1, 32),
            Self::SixtyFourth => Fraction::new(1, 64),
            Self::OneHundredTwentyEighth => Fraction::new(1, 128),
            Self::TwoHundredFiftySixth => Fraction::new(1, 256),
            Self::FiveHundredTwelfth => Fraction::new(1, 512),
            Self::OneThousandTwentyFourth => Fraction::new(1, 1024),
            Self::Zero | Self::Measure => Fraction::ZERO,
            Self::Invalid => Fraction::from_integer(-1),
        }
    }

    /// Tête de note associée par défaut, comme `TDuration::headType()`.
    pub const fn head_type(self) -> NoteHeadType {
        match self {
            Self::Breve | Self::Long => NoteHeadType::Breve,
            Self::Whole | Self::Measure => NoteHeadType::Whole,
            Self::Half => NoteHeadType::Half,
            Self::Invalid
            | Self::Zero
            | Self::Quarter
            | Self::Eighth
            | Self::Sixteenth
            | Self::ThirtySecond
            | Self::SixtyFourth
            | Self::OneHundredTwentyEighth
            | Self::TwoHundredFiftySixth
            | Self::FiveHundredTwelfth
            | Self::OneThousandTwentyFourth => NoteHeadType::Quarter,
        }
    }

    /// Nombre de crochets, comme `TDuration::hooks()`.
    pub const fn hooks(self) -> u8 {
        match self {
            Self::Eighth => 1,
            Self::Sixteenth => 2,
            Self::ThirtySecond => 3,
            Self::SixtyFourth => 4,
            Self::OneHundredTwentyEighth => 5,
            Self::TwoHundredFiftySixth => 6,
            Self::FiveHundredTwelfth => 7,
            Self::OneThousandTwentyFourth => 8,
            _ => 0,
        }
    }

    /// Vrai si la figure comporte une hampe.
    pub const fn has_stem(self) -> bool {
        matches!(
            self,
            Self::Half
                | Self::Quarter
                | Self::Eighth
                | Self::Sixteenth
                | Self::ThirtySecond
                | Self::SixtyFourth
                | Self::OneHundredTwentyEighth
                | Self::TwoHundredFiftySixth
                | Self::FiveHundredTwelfth
                | Self::OneThousandTwentyFourth
        )
    }

    /// Vrai pour un silence de mesure entière.
    pub const fn is_measure(self) -> bool {
        matches!(self, Self::Measure)
    }
}

/// Figure et points de prolongation.
///
/// Correspond à `TDuration` dans `engraving/dom/durationtype.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Duration {
    /// Figure de base.
    pub kind: DurationType,
    /// Nombre de points (0 pour une figure non pointée).
    pub dots: u8,
}

impl Duration {
    /// Construit une durée non pointée.
    pub const fn new(kind: DurationType) -> Self {
        Self { kind, dots: 0 }
    }

    /// Construit une durée avec un nombre donné de points.
    pub const fn with_dots(kind: DurationType, dots: u8) -> Self {
        Self { kind, dots }
    }

    /// Ronde.
    pub const fn whole() -> Self {
        Self::new(DurationType::Whole)
    }

    /// Blanche.
    pub const fn half() -> Self {
        Self::new(DurationType::Half)
    }

    /// Noire.
    pub const fn quarter() -> Self {
        Self::new(DurationType::Quarter)
    }

    /// Croche.
    pub const fn eighth() -> Self {
        Self::new(DurationType::Eighth)
    }

    /// Silence de mesure entière.
    pub const fn measure() -> Self {
        Self::new(DurationType::Measure)
    }

    /// Durée totale en fractions de ronde, points compris.
    ///
    /// La boucle reproduit `TDuration::ticks()` : chaque point ajoute la moitié
    /// de l'ajout précédent (`1 + 1/2 + 1/4 + …`).
    pub fn ticks(self) -> Fraction {
        let base = self.kind.ticks();
        if self.kind.is_measure() || matches!(self.kind, DurationType::Zero | DurationType::Invalid)
        {
            return base;
        }

        let mut total = base;
        let mut increment = base * Fraction::HALF;
        for _ in 0..self.dots {
            total += increment;
            increment *= Fraction::HALF;
        }
        total
    }

    /// Vrai si la durée porte au moins un point.
    pub const fn is_dotted(self) -> bool {
        self.dots > 0
    }

    /// Vrai pour un silence de mesure entière.
    pub const fn is_measure(self) -> bool {
        self.kind.is_measure()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_de_base() {
        assert_eq!(DurationType::Whole.ticks(), Fraction::ONE);
        assert_eq!(DurationType::Quarter.ticks(), Fraction::QUARTER);
        assert_eq!(DurationType::Eighth.ticks(), Fraction::new(1, 8));
        assert_eq!(DurationType::Breve.ticks(), Fraction::from_integer(2));
        assert_eq!(DurationType::Long.ticks(), Fraction::from_integer(4));
        assert_eq!(DurationType::Measure.ticks(), Fraction::ZERO);
        assert_eq!(DurationType::Invalid.ticks(), Fraction::from_integer(-1));
    }

    #[test]
    fn points_de_prolongation() {
        assert_eq!(
            Duration::with_dots(DurationType::Half, 1).ticks(),
            Fraction::new(3, 4)
        );
        assert_eq!(
            Duration::with_dots(DurationType::Half, 2).ticks(),
            Fraction::new(7, 8)
        );
        assert_eq!(
            Duration::with_dots(DurationType::Quarter, 1).ticks(),
            Fraction::new(3, 8)
        );
        assert_eq!(Duration::quarter().ticks(), Fraction::QUARTER);
        assert_eq!(Duration::measure().ticks(), Fraction::ZERO);
    }

    #[test]
    fn crochets_et_tetes() {
        assert_eq!(DurationType::Quarter.hooks(), 0);
        assert_eq!(DurationType::Eighth.hooks(), 1);
        assert_eq!(DurationType::OneHundredTwentyEighth.hooks(), 5);
        assert_eq!(DurationType::Whole.head_type(), NoteHeadType::Whole);
        assert_eq!(DurationType::Breve.head_type(), NoteHeadType::Breve);
        assert_eq!(DurationType::Eighth.head_type(), NoteHeadType::Quarter);
        assert!(DurationType::Quarter.has_stem());
        assert!(!DurationType::Whole.has_stem());
    }
}
