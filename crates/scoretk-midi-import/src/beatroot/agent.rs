// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Agent.h` / `Agent.cpp` du plug-in BeatRoot (Vamp, Queen Mary
// University of London), copyright 2011 Simon Dixon, Chris Cannam et QMUL,
// licence GPL-2.0-ou-ultérieure, modifié (réécrit en Rust) en 2026. Référence :
// `src/importexport/midi/thirdparty/beatroot/Agent.{h,cpp}`.
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

//! Agent de suivi de tempo (`Agent.cpp`).

use std::sync::atomic::{AtomicI64, Ordering};

use super::agent_list::AgentList;
use super::event::Event;

static ID_COUNTER: AtomicI64 = AtomicI64::new(0);

/// `AgentParameters` (`Agent.h:30-54`).
#[derive(Clone, Copy, Debug)]
pub struct AgentParameters {
    pub post_margin_factor: f64,
    pub pre_margin_factor: f64,
    pub max_change: f64,
    pub expiry_time: f64,
}

impl Default for AgentParameters {
    fn default() -> Self {
        Self {
            post_margin_factor: 0.3,
            pre_margin_factor: 0.15,
            max_change: 0.2,
            expiry_time: 10.0,
        }
    }
}

const INNER_MARGIN: f64 = 0.040;
const CONF_FACTOR: f64 = 0.5;
const DEFAULT_CORRECTION_FACTOR: f64 = 50.0;

/// `Agent` (`Agent.h:64-204`, `Agent.cpp:36-116`).
#[derive(Clone, Debug)]
pub struct Agent {
    pub id_number: i64,
    pub phase_score: f64,
    pub beat_count: i32,
    pub beat_interval: f64,
    pub initial_beat_interval: f64,
    pub beat_time: f64,
    pub max_change: f64,
    pub events: Vec<Event>,
    pre_margin: f64,
    post_margin: f64,
    inner_margin: f64,
    correction_factor: f64,
    expiry_time: f64,
}

impl Agent {
    /// `Agent::Agent` (`Agent.cpp:36-53`).
    pub fn new(params: AgentParameters, ibi: f64) -> Self {
        Self {
            id_number: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            phase_score: 0.0,
            beat_count: 0,
            beat_interval: ibi,
            initial_beat_interval: ibi,
            beat_time: -1.0,
            max_change: params.max_change,
            events: Vec::new(),
            pre_margin: ibi * params.pre_margin_factor,
            post_margin: ibi * params.post_margin_factor,
            inner_margin: INNER_MARGIN,
            correction_factor: DEFAULT_CORRECTION_FACTOR,
            expiry_time: params.expiry_time,
        }
    }

    /// `Agent::clone` (`Agent.cpp:55-59`) : nouvelle identité.
    pub fn fork(&self) -> Self {
        let mut agent = self.clone();
        agent.id_number = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        agent
    }

    /// `Agent::accept` (`Agent.cpp:61-73`).
    pub fn accept(&mut self, event: Event, err: f64, beats: i32) {
        self.beat_time = event.time;
        self.events.push(event);
        if (self.initial_beat_interval - self.beat_interval - err / self.correction_factor).abs()
            < self.max_change * self.initial_beat_interval
        {
            self.beat_interval += err / self.correction_factor;
        }
        self.beat_count += beats;
        let denom = if err > 0.0 {
            self.post_margin
        } else {
            -self.pre_margin
        };
        let conf_factor = 1.0 - CONF_FACTOR * err / denom;
        self.phase_score += conf_factor * event.salience;
    }

    /// `Agent::considerAsBeat` (`Agent.cpp:75-102`).
    pub fn consider_as_beat(&mut self, event: Event, agents: &mut AgentList) -> bool {
        if self.beat_time < 0.0 {
            self.accept(event, 0.0, 1);
            return true;
        }
        let Some(last) = self.events.last() else {
            return false;
        };
        if event.time - last.time > self.expiry_time {
            self.phase_score = -1.0;
            return false;
        }
        let beats = (event.time - self.beat_time) / self.beat_interval;
        let beats_i = beats.round();
        let err = event.time - self.beat_time - beats_i * self.beat_interval;
        if beats_i > 0.0 && -self.pre_margin <= err && err <= self.post_margin {
            if err.abs() > self.inner_margin {
                agents.add(self.fork());
            }
            self.accept(event, err, beats_i as i32);
            return true;
        }
        false
    }

    /// `Agent::fillBeats` (`Agent.cpp:104-116`).
    pub fn fill_beats(&mut self, start: f64) {
        if self.events.is_empty() {
            return;
        }
        let mut prev_beat = self.events[0].time;
        let mut index = 1;
        while index < self.events.len() {
            let next_beat = self.events[index].time;
            let mut beats = ((next_beat - prev_beat) / self.beat_interval - 0.01).round();
            let current_interval = (next_beat - prev_beat) / beats;
            while next_beat > start && beats > 1.5 {
                prev_beat += current_interval;
                // `events.insert(it, …)` insère avant l'itérateur : l'élément
                // courant reste `index`, on ne l'avance surtout pas ici.
                self.events.insert(index, Event::new(prev_beat, 0.0));
                beats -= 1.0;
            }
            prev_beat = next_beat;
            index += 1;
        }
    }
}
