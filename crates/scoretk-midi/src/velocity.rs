// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/velocitymap.cpp` de MuseScore Studio,
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

//! Carte des vélocités (nuances et soufflets).
//!
//! Portage de `VelocityMap` (`engraving/compat/midi/velocitymap.cpp`) : chaque
//! piste (portée × voix) porte une suite d'évènements de nuance et de soufflet,
//! et `val(tick)` donne la vélocité en vigueur, interpolée dans les soufflets.
//! Les ticks sont ceux de MuseScore, en unités par ronde ([`UNITS_PER_WHOLE`]).

use std::collections::BTreeMap;

/// Valeur par défaut quand aucun évènement ne précède le tick
/// (`VelocityMap::DEFAULT_VALUE`).
pub(crate) const DEFAULT_VALUE: i32 = 80;

const MIN_VALUE: i32 = 1;
const MAX_VALUE: i32 = 127;
const STEP: i32 = 16;

/// Méthode d'interpolation d'un soufflet (`ChangeMethod`).
///
/// Toutes les méthodes de MuseScore sont portées ; le modèle ne stocke pas
/// encore la méthode d'un soufflet, l'export n'utilise donc que [`Normal`].
///
/// [`Normal`]: ChangeMethod::Normal
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeMethod {
    Normal,
    Exponential,
    EaseInOut,
    EaseIn,
    EaseOut,
}

/// Sens d'évolution d'un soufflet (`ChangeDirection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeDirection {
    Increasing,
    Decreasing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventKind {
    Dynamic,
    Hairpin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Event {
    value: i32,
    kind: EventKind,
    length: i64,
    method: ChangeMethod,
    direction: ChangeDirection,
    cached_start: i32,
    cached_end: i32,
}

impl Event {
    fn dynamic(value: i32) -> Self {
        Self {
            value,
            kind: EventKind::Dynamic,
            length: 0,
            method: ChangeMethod::Normal,
            direction: ChangeDirection::Increasing,
            cached_start: -1,
            cached_end: -1,
        }
    }

    fn hairpin(
        start: i64,
        end: i64,
        change: i32,
        method: ChangeMethod,
        direction: ChangeDirection,
    ) -> Self {
        Self {
            value: change,
            kind: EventKind::Hairpin,
            length: end - start,
            method,
            direction,
            cached_start: -1,
            cached_end: -1,
        }
    }
}

/// Carte des vélocités d'une piste.
#[derive(Clone, Debug, Default)]
pub(crate) struct VelocityMap {
    events: BTreeMap<i64, Vec<Event>>,
}

impl VelocityMap {
    fn insert(&mut self, tick: i64, event: Event) {
        self.events.entry(tick).or_default().push(event);
    }

    /// Vélocité en vigueur au tick donné (`VelocityMap::val`).
    pub(crate) fn val(&self, tick: i64) -> i32 {
        let Some((start, at_tick)) = self.events.range(..=tick).next_back() else {
            return DEFAULT_VALUE;
        };
        if let Some(hairpin) = at_tick
            .iter()
            .rev()
            .find(|event| event.kind == EventKind::Hairpin)
        {
            if tick >= start + hairpin.length {
                return hairpin.cached_end;
            }
            return interpolate(*start, hairpin, tick);
        }
        at_tick.last().map_or(DEFAULT_VALUE, |event| event.value)
    }

    /// Ajoute une nuance (`addDynamic`).
    pub(crate) fn add_dynamic(&mut self, tick: i64, value: i32) {
        self.insert(tick, Event::dynamic(value));
    }

    /// Ajoute un soufflet (`addHairpin`). `change == 0` signifie que la valeur
    /// d'arrivée se déduit de la nuance suivante.
    pub(crate) fn add_hairpin(
        &mut self,
        start: i64,
        end: i64,
        change: i32,
        method: ChangeMethod,
        direction: ChangeDirection,
    ) {
        let change = change.abs()
            * match direction {
                ChangeDirection::Increasing => 1,
                ChangeDirection::Decreasing => -1,
            };
        self.insert(start, Event::hairpin(start, end, change, method, direction));
    }

    /// Prépare la carte (`VelocityMap::setup`).
    pub(crate) fn setup(&mut self) {
        self.sort_hairpins();
        self.resolve_hairpin_collisions();
        self.resolve_dynamic_inside_hairpin_collisions();
        self.add_missing_dynamics_after_hairpins();
        self.fill_hairpins_cache();
    }

    fn flatten(&self) -> Vec<(i64, Event)> {
        let mut flat = Vec::new();
        for (tick, events) in &self.events {
            for event in events {
                flat.push((*tick, event.clone()));
            }
        }
        flat
    }

    fn rebuild(&mut self, flat: Vec<(i64, Event)>) {
        self.events.clear();
        for (tick, event) in flat {
            self.insert(tick, event);
        }
    }

    /// Trie les soufflets d'un même tick du plus long au plus court
    /// (`sortHairpins`).
    fn sort_hairpins(&mut self) {
        let keys: Vec<i64> = self.events.keys().copied().collect();
        for key in keys {
            let Some(events) = self.events.get_mut(&key) else {
                continue;
            };
            let mut hairpins: Vec<Event> = events
                .iter()
                .filter(|event| event.kind == EventKind::Hairpin)
                .cloned()
                .collect();
            if hairpins.len() <= 1 {
                continue;
            }
            hairpins.sort_by_key(|hairpin| std::cmp::Reverse(hairpin.length));
            events.retain(|event| event.kind == EventKind::Dynamic);
            events.extend(hairpins);
        }
    }

    /// Retire les soufflets entièrement contenus dans un autre
    /// (`resolveHairpinCollisions`).
    fn resolve_hairpin_collisions(&mut self) {
        let flat = self.flatten();
        let mut kept = Vec::with_capacity(flat.len());
        let mut end_points: Vec<(i64, i64)> = Vec::new();
        let mut starts_in_hairpin: Vec<bool> = Vec::new();
        let mut current_end = i64::MIN;
        let mut in_hairpin = false;

        for (tick, event) in flat {
            let end = tick + event.length;
            if current_end < tick {
                in_hairpin = false;
            }
            if event.kind == EventKind::Hairpin {
                if in_hairpin {
                    if end <= current_end {
                        // Enveloppé : supprimé.
                        continue;
                    }
                    current_end = end;
                    starts_in_hairpin.push(true);
                } else {
                    current_end = end;
                    in_hairpin = true;
                    starts_in_hairpin.push(false);
                }
                end_points.push((tick, end));
            }
            kept.push((tick, event));
        }

        self.rebuild(kept);
        self.adjust_colliding_hairpins_length(&starts_in_hairpin, &end_points);
    }

    /// Raccourcit les soufflets qui commencent dans un autre
    /// (`adjustCollidingHairpinsLength`).
    fn adjust_colliding_hairpins_length(
        &mut self,
        starts_in_hairpin: &[bool],
        end_points: &[(i64, i64)],
    ) {
        let flat = self.flatten();
        let mut kept = Vec::with_capacity(flat.len());
        let mut moved: Vec<(i64, Event)> = Vec::new();
        let mut hairpin_index: i32 = -1;
        for (tick, mut event) in flat {
            if event.kind != EventKind::Hairpin {
                kept.push((tick, event));
                continue;
            }
            hairpin_index += 1;
            let index = hairpin_index as usize;
            if !starts_in_hairpin.get(index).copied().unwrap_or(false) {
                kept.push((tick, event));
                continue;
            }
            let new_tick = end_points[index - 1].1;
            event.length -= new_tick - tick;
            moved.push((new_tick, event));
        }
        self.rebuild(kept);
        moved.sort_by_key(|(tick, _)| *tick);
        for (tick, event) in moved {
            self.insert(tick, event);
        }
    }

    /// Raccourcit un soufflet qui contient une nuance
    /// (`resolveDynamicInsideHairpinCollisions`).
    fn resolve_dynamic_inside_hairpin_collisions(&mut self) {
        let mut flat = self.flatten();
        let mut last_hairpin: Option<usize> = None;
        for index in 0..flat.len() {
            if flat[index].1.kind == EventKind::Dynamic {
                if let Some(last) = last_hairpin {
                    let dynamic_tick = flat[index].0;
                    let hairpin_start = flat[last].0;
                    let hairpin_end = hairpin_start + flat[last].1.length;
                    if hairpin_end > dynamic_tick {
                        flat[last].1.length = dynamic_tick - hairpin_start;
                    }
                }
            }
            last_hairpin = if flat[index].1.kind == EventKind::Hairpin {
                Some(index)
            } else {
                last_hairpin
            };
        }
        self.rebuild(flat);
    }

    fn dynamic_exists_on_tick(flat: &[(i64, Event)], tick: i64) -> bool {
        flat.iter()
            .any(|(at, event)| *at == tick && event.kind == EventKind::Dynamic)
    }

    fn dynamic_event_for_tick(flat: &[(i64, Event)], tick: i64) -> Option<(i64, i32)> {
        if let Some((at, event)) = flat
            .iter()
            .find(|(at, event)| *at == tick && event.kind == EventKind::Dynamic)
        {
            return Some((*at, event.value));
        }
        // Sinon, la dernière nuance d'un tick antérieur.
        flat.iter()
            .rev()
            .find(|(at, event)| *at < tick && event.kind == EventKind::Dynamic)
            .map(|(at, event)| (*at, event.value))
    }

    /// Ajoute la nuance manquante à la fin d'un soufflet, ou corrige un saut
    /// dans le mauvais sens (`addMissingDynamicsAfterHairpins`).
    fn add_missing_dynamics_after_hairpins(&mut self) {
        let mut flat = self.flatten();
        let mut extra: Vec<(i64, Event)> = Vec::new();
        for index in 0..flat.len() {
            if flat[index].1.kind != EventKind::Hairpin || flat[index].1.value != 0 {
                continue;
            }
            let start = flat[index].0;
            let end = start + flat[index].1.length;
            let direction = flat[index].1.direction;
            let Some((_, start_value)) = Self::dynamic_event_for_tick(&flat, start) else {
                continue;
            };
            let next_value = match direction {
                ChangeDirection::Increasing => (start_value + STEP).clamp(MIN_VALUE, MAX_VALUE),
                ChangeDirection::Decreasing => (start_value - STEP).clamp(MIN_VALUE, MAX_VALUE),
            };
            if !Self::dynamic_exists_on_tick(&flat, end) {
                extra.push((end, Event::dynamic(next_value)));
            } else if let Some((_, end_value)) = Self::dynamic_event_for_tick(&flat, end) {
                let jump = end_value - start_value;
                if (direction == ChangeDirection::Increasing && jump < 0)
                    || (direction == ChangeDirection::Decreasing && jump > 0)
                {
                    flat[index].1.value = next_value - start_value;
                }
            }
        }
        if !extra.is_empty() {
            flat.extend(extra);
            flat.sort_by_key(|(tick, _)| *tick);
            self.rebuild(flat);
        } else {
            self.rebuild(flat);
        }
    }

    /// Cache les valeurs de début et de fin de chaque soufflet
    /// (`fillHairpinsCache`).
    fn fill_hairpins_cache(&mut self) {
        let mut flat = self.flatten();
        for index in 0..flat.len() {
            if flat[index].1.kind != EventKind::Hairpin {
                continue;
            }
            let tick = flat[index].0;

            // Phase 1 : valeur de départ.
            let mut start = None;
            for (at, event) in &flat {
                if *at == tick && event.kind == EventKind::Dynamic {
                    start = Some(event.value);
                    break;
                }
            }
            let start = start.unwrap_or_else(|| {
                if index == 0 {
                    DEFAULT_VALUE
                } else {
                    let (previous_tick, previous) = &flat[index - 1];
                    if previous.kind == EventKind::Hairpin {
                        previous.cached_end
                    } else {
                        // Le dernier évènement du tick précédent peut être un
                        // soufflet inséré après la nuance.
                        flat.iter()
                            .rev()
                            .find(|(at, event)| {
                                at == previous_tick && event.kind == EventKind::Hairpin
                            })
                            .map_or(previous.value, |(_, event)| event.cached_end)
                    }
                }
            });

            // Phase 2 : valeur d'arrivée.
            let end = if flat[index].1.value != 0 {
                start + flat[index].1.value
            } else {
                let mut next = index + 1;
                while next < flat.len() && flat[next].0 == tick {
                    next += 1;
                }
                if next >= flat.len() {
                    start
                } else {
                    let next_tick = flat[next].0;
                    flat.iter()
                        .find(|(at, event)| *at == next_tick && event.kind == EventKind::Dynamic)
                        .map_or(start, |(_, event)| event.value)
                }
            };

            let end = if (start > end && flat[index].1.direction == ChangeDirection::Increasing)
                || (start < end && flat[index].1.direction == ChangeDirection::Decreasing)
            {
                start
            } else {
                end
            };
            flat[index].1.cached_start = start;
            flat[index].1.cached_end = end;
        }
        self.rebuild(flat);
    }
}

/// Interpolation dans un soufflet (`VelocityMap::interpolate`).
fn interpolate(event_tick: i64, event: &Event, tick: i64) -> i32 {
    if event.cached_start == event.cached_end || event.length == 0 {
        return event.cached_start;
    }
    let expr_ticks = event.length as f64;
    let diff = (event.cached_end - event.cached_start) as f64;
    let ct = (tick - event_tick) as f64;
    let value = match event.method {
        ChangeMethod::Exponential => {
            if diff > 0.0 {
                ((diff + 1.0).powf(1.0 / expr_ticks)).powf(ct) - 1.0
            } else {
                -(((-diff + 1.0).powf(1.0 / expr_ticks)).powf(ct) + 1.0)
            }
        }
        ChangeMethod::EaseInOut => {
            (diff / 2.0)
                * ((ct * (std::f64::consts::PI / expr_ticks) - std::f64::consts::PI / 2.0).sin()
                    + 1.0)
        }
        ChangeMethod::EaseIn => {
            diff * ((ct - expr_ticks) * (std::f64::consts::PI / (2.0 * expr_ticks))).sin() + 1.0
        }
        ChangeMethod::EaseOut => diff * (ct * (std::f64::consts::PI / (2.0 * expr_ticks))).sin(),
        ChangeMethod::Normal => diff * (ct / expr_ticks),
    };
    event.cached_start + value as i32
}

/// Carte des vélocités par piste (`Context::velocitiesByTrack`).
#[derive(Clone, Debug, Default)]
pub(crate) struct VelocityMaps {
    maps: BTreeMap<u32, VelocityMap>,
}

impl VelocityMaps {
    /// Carte d'une piste, créée à la demande.
    pub(crate) fn map_mut(&mut self, track: u32) -> &mut VelocityMap {
        self.maps.entry(track).or_default()
    }

    /// Vélocité au tick donné pour une piste.
    pub(crate) fn val(&self, track: u32, tick: i64) -> i32 {
        self.maps
            .get(&track)
            .map_or(DEFAULT_VALUE, |map| map.val(tick))
    }

    /// Prépare toutes les cartes (`VelocityMap::setup`).
    pub(crate) fn setup(&mut self) {
        for map in self.maps.values_mut() {
            map.setup();
        }
    }
}
