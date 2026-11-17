// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
//
// Portage de `engraving/dom/tuplet.h` de MuseScore Studio, Copyright (C)
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

//! N-olets.
//!
//! Portage de `Tuplet` (`engraving/dom/tuplet.h`). Le ratio suit la convention
//! de MuseScore : un triolet de croches a `ratio = 3/2`, `base_len = 1/8`, et
//! chaque croche du triolet dure `base_len * denominator / numerator`.
//!
//! Les n-olets imbriqués sont représentés par [`Tuplet::parent`] ; l'accord ou
//! le silence ne référence que le n-olet le plus interne via
//! [`super::ChordRest::tuplet`].

use super::annotation::Placement;
use super::fraction::{Fraction, Tick};
use super::ids::TupletId;

/// Affichage du nombre d'un n-olet.
///
/// Correspond à `TupletNumberType` dans `engraving/dom/tuplet.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TupletNumberType {
    /// Afficher le nombre (`3`).
    #[default]
    Number,
    /// Afficher le ratio (`3:2`).
    Ratio,
    /// Ne rien afficher.
    None,
}

/// N-olet.
///
/// Correspond à `Tuplet` dans `engraving/dom/tuplet.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tuplet {
    /// Ratio `nombre / normal`, par exemple `3/2` pour un triolet.
    pub ratio: Fraction,
    /// Durée de base notée, par exemple `1/8` pour un triolet de croches.
    pub base_len: Fraction,
    /// Tick de début.
    pub tick: Tick,
    /// Durée totale du n-olet.
    pub len: Fraction,
    /// N-olet parent si celui-ci est imbriqué.
    pub parent: Option<TupletId>,
    /// Type d'affichage du nombre.
    pub number_type: TupletNumberType,
    /// Vrai si le n-olet est encadré d'un crochet.
    pub bracket: bool,
    /// Nombre de notes jouées tel qu'écrit dans le fichier (`<actualNotes>`).
    ///
    /// Le ratio est réduit, donc `4/4` y devient `1/1` ; MuseScore affiche en
    /// revanche les nombres bruts dans `<tuplet-actual>`/`<tuplet-normal>`.
    /// Vaut le numérateur du ratio pour un n-olet construit à la main.
    pub actual_notes: i64,
    /// Nombre de notes normales tel qu'écrit (`<normalNotes>`), non réduit.
    pub normal_notes: i64,
    /// Placement explicite du nombre (`<direction>`), exporté en `placement`.
    ///
    /// `None` correspond à `DirectionV::AUTO` : MuseScore n'écrit alors pas
    /// d'attribut `placement`.
    pub placement: Option<Placement>,
}

impl Tuplet {
    /// Construit un n-olet.
    pub fn new(ratio: Fraction, base_len: Fraction, tick: Tick, len: Fraction) -> Self {
        Self {
            ratio,
            base_len,
            tick,
            len,
            parent: None,
            number_type: TupletNumberType::Number,
            bracket: false,
            actual_notes: ratio.numerator(),
            normal_notes: ratio.denominator(),
            placement: None,
        }
    }

    /// Nombres affichés, non réduits (`<actualNotes>` et `<normalNotes>`).
    pub const fn displayed_numbers(&self) -> (i64, i64) {
        (self.actual_notes, self.normal_notes)
    }

    /// Nombre affiché du n-olet (numérateur du ratio).
    pub fn number(&self) -> i64 {
        self.ratio.numerator()
    }

    /// Tick de fin (exclu).
    pub fn end_tick(&self) -> Tick {
        self.tick + self.len
    }
}
