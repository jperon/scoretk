// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/sig.cpp` (`TimeSigMap`) de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Carte des indications de mesure (`TimeSigMap`).
//!
//! Portage de `TimeSigMap` (`engraving/dom/sig.cpp`) : les ticks y sont
//! exprimés en unités de 1/480 de noire, comme `Constants::DIVISION`. La carte
//! précalcule le numéro de mesure de chaque changement (`normalize`), ce qui
//! rend `bar2tick` et `tickValues` immédiats.
//!
//! Contrairement à [`Fraction`] du modèle, une indication de mesure n'est
//! **pas** réduite : 4/4 et 1/1 ont la même durée mais pas le même affichage,
//! et MuseScore conserve les deux nombres.

use std::collections::BTreeMap;

use mscz2xml::model::Fraction;

use super::midifile::UNITS_PER_WHOLE;

/// Ticks par temps pour un dénominateur d'indication de mesure.
///
/// `ticks_beat` de `sig.cpp` : `DIVISION * 4 / n`.
pub fn ticks_beat(denominator: i64) -> i64 {
    if denominator <= 0 {
        return 0;
    }
    UNITS_PER_WHOLE / denominator
}

/// Indication de mesure non réduite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timesig {
    /// Numérateur (nombre de temps).
    pub numerator: i64,
    /// Dénominataur (figure du temps).
    pub denominator: i64,
}

impl Timesig {
    /// Construit une indication de mesure.
    pub const fn new(numerator: i64, denominator: i64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Longueur de la mesure en fraction de ronde (réduite, pour les calculs).
    pub fn measure_len(self) -> Fraction {
        Fraction::try_new(self.numerator, self.denominator).unwrap_or(Fraction::ZERO)
    }

    /// Nombre de ticks de la mesure (`ticks_measure`).
    pub fn ticks_measure(self) -> i64 {
        ticks_beat(self.denominator) * self.numerator
    }
}

impl Default for Timesig {
    fn default() -> Self {
        Self::new(4, 4)
    }
}

/// Ticks d'une mesure de l'indication donnée (`ticks_measure`).
pub fn ticks_measure(timesig: Timesig) -> i64 {
    timesig.ticks_measure()
}

/// Indication de mesure enregistrée à un tick.
#[derive(Clone, Copy, Debug)]
pub struct SigEvent {
    /// Indication de mesure.
    pub timesig: Timesig,
    /// Numéro de mesure de ce changement, précalculé par `normalize`.
    pub bar: i64,
}

/// Carte des indications de mesure.
#[derive(Clone, Debug, Default)]
pub struct TimeSigMap {
    events: BTreeMap<i64, SigEvent>,
}

impl TimeSigMap {
    /// Construit une carte vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Vrai si aucun changement n'est enregistré.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Enregistre une indication de mesure au tick donné (`TimeSigMap::add`).
    pub fn add(&mut self, tick: i64, timesig: Timesig) {
        if timesig.numerator <= 0 || timesig.denominator <= 0 {
            return;
        }
        self.events.insert(tick, SigEvent { timesig, bar: 0 });
        self.normalize();
    }

    /// Recalcule le numéro de mesure de chaque changement (`normalize`).
    fn normalize(&mut self) {
        let mut tick = 0_i64;
        let mut bar = Fraction::ZERO;
        let mut tm = Timesig::default().ticks_measure();
        let ticks: Vec<i64> = self.events.keys().copied().collect();
        for event_tick in ticks {
            let delta = event_tick - tick;
            if tm > 0 {
                bar += Fraction::new(delta, tm);
            }
            let bar_number = bar.numerator() / bar.denominator();
            if let Some(event) = self.events.get_mut(&event_tick) {
                event.bar = bar_number;
                tm = event.timesig.ticks_measure();
            }
            tick = event_tick;
        }
    }

    /// Indication de mesure en vigueur au tick donné (`TimeSigMap::timesig`).
    ///
    /// Renvoie 4/4 quand la carte est vide, comme la valeur par défaut posée
    /// par `createMTrackList`.
    pub fn timesig(&self, tick: i64) -> Timesig {
        self.events
            .range(..=tick)
            .next_back()
            .map(|(_, event)| event.timesig)
            .unwrap_or_default()
    }

    /// Indication de mesure en vigueur et numéro de mesure au tick donné.
    pub fn timesig_and_bar(&self, tick: i64) -> (Timesig, i64) {
        match self.events.range(..=tick).next_back() {
            Some((event_tick, event)) => {
                let delta = tick - event_tick;
                let tm = event.timesig.ticks_measure();
                let bar = if tm > 0 { event.bar + delta / tm } else { event.bar };
                (event.timesig, bar)
            }
            None => (Timesig::default(), 0),
        }
    }

    /// Numéro de mesure (depuis zéro) du tick (`TimeSigMap::tickValues`).
    pub fn bar_number(&self, tick: i64) -> i64 {
        self.timesig_and_bar(tick).1
    }

    /// Position d'un tick en mesure, temps et tick (`tickValues`).
    pub fn tick_values(&self, tick: i64) -> (i64, i64, i64) {
        let Some((event_tick, event)) = self.events.range(..=tick).next_back() else {
            return (0, 0, 0);
        };
        let delta = tick - event_tick;
        let ticks_b = ticks_beat(event.timesig.denominator);
        let ticks_m = event.timesig.ticks_measure();
        if ticks_m == 0 {
            return (0, 0, 0);
        }
        let bar = event.bar + delta / ticks_m;
        let rest = delta % ticks_m;
        (bar, rest / ticks_b, rest % ticks_b)
    }

    /// Tick du début de la mesure `bar`, au temps `beat` (`bar2tick`).
    pub fn bar2tick(&self, bar: i64, beat: i64) -> i64 {
        let mut current: Option<(i64, SigEvent)> = None;
        for (event_tick, event) in &self.events {
            if bar < event.bar {
                break;
            }
            current = Some((*event_tick, *event));
        }
        let Some((event_tick, event)) = current else {
            return 0;
        };
        let ticks_b = ticks_beat(event.timesig.denominator);
        let ticks_m = event.timesig.ticks_measure();
        event_tick + (bar - event.bar) * ticks_m + ticks_b * beat
    }

    /// Itère sur les changements dans l'ordre des ticks.
    pub fn iter(&self) -> impl Iterator<Item = (i64, SigEvent)> + '_ {
        self.events.iter().map(|(tick, event)| (*tick, *event))
    }
}

/// Convertit des ticks MuseScore (480 par noire) en fraction de ronde.
pub fn fraction_from_ticks(ticks: i64) -> Fraction {
    Fraction::from_ticks(ticks, UNITS_PER_WHOLE as u32)
}
