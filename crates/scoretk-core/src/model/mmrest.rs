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

//! Silences multi-mesures portés par une mesure, **par portée**.
//!
//! Portage de `Measure::isEmpty(staffIdx)` (`engraving/dom/measure.cpp:2286-2330`)
//! et de `MMRestLayout::createMMRest` (`engraving/rendering/score/mmrestlayout.cpp:161-206`) :
//! MuseScore regroupe les mesures vides pour une portée donnée ; une même
//! mesure peut donc ouvrir un `<multiple-rest>` sur une portée et porter des
//! notes sur une autre. Le champ historique `Measure::multi_measure_rest`
//! reste la valeur globale utilisée par les autres écrivains ; celui-ci
//! conserve l'information par portée lue dans les fichiers MuseScore.

use super::StaffId;

/// Nombre de mesures regroupées par un silence multi-mesures, par portée.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultiMeasureRests {
    /// Paires `(portée, nombre)`, triées par portée.
    entries: Vec<(StaffId, u16)>,
}

impl MultiMeasureRests {
    /// Nouveau groupe vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre le nombre de mesures regroupées pour `staff`.
    pub fn set(&mut self, staff: StaffId, count: u16) {
        if count == 0 {
            self.remove(staff);
            return;
        }
        match self.entries.binary_search_by_key(&staff, |(id, _)| *id) {
            Ok(index) => self.entries[index].1 = count,
            Err(index) => self.entries.insert(index, (staff, count)),
        }
    }

    /// Retire l'entrée de `staff`, si elle existe.
    pub fn remove(&mut self, staff: StaffId) {
        if let Ok(index) = self.entries.binary_search_by_key(&staff, |(id, _)| *id) {
            self.entries.remove(index);
        }
    }

    /// Nombre de mesures regroupées pour `staff`, s'il y en a.
    pub fn count_for(&self, staff: StaffId) -> Option<u16> {
        self.entries
            .binary_search_by_key(&staff, |(id, _)| *id)
            .ok()
            .map(|index| self.entries[index].1)
    }

    /// Première valeur enregistrée, pour les écrivains qui n'ont pas de portée.
    pub fn primary(&self) -> Option<u16> {
        self.entries.first().map(|(_, count)| *count)
    }

    /// Vrai si aucune portée n'ouvre de multi-repos ici.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Itère sur les paires `(portée, nombre)`.
    pub fn iter(&self) -> impl Iterator<Item = (StaffId, u16)> + '_ {
        self.entries.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_repos_sont_ranges_par_portee() {
        let mut rests = MultiMeasureRests::new();
        let a = StaffId::from_index(3);
        let b = StaffId::from_index(1);
        rests.set(a, 4);
        rests.set(b, 2);
        rests.set(a, 5);
        rests.set(b, 0);
        assert_eq!(rests.count_for(a), Some(5));
        assert_eq!(rests.count_for(b), None);
        assert_eq!(rests.primary(), Some(5));
        assert_eq!(rests.iter().count(), 1);
    }
}
