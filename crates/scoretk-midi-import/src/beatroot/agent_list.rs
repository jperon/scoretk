// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `AgentList.h` / `AgentList.cpp` du plug-in BeatRoot (Vamp, Queen
// Mary University of London), copyright 2011 Simon Dixon, Chris Cannam et
// QMUL, licence GPL-2.0-ou-ultérieure, modifié (réécrit en Rust) en 2026.
// Référence : `src/importexport/midi/thirdparty/beatroot/AgentList.{h,cpp}`.
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

//! Liste d'agents (`AgentList.cpp`).

use super::agent::{Agent, AgentParameters};
use super::event::Event;

/// Juste-noticeable difference de tempo et de phase (`AgentList.cpp:22-23`).
const DEFAULT_BI: f64 = 0.02;
const DEFAULT_BT: f64 = 0.04;

/// `AgentList` (`AgentList.h:35-102`).
#[derive(Debug, Default)]
pub struct AgentList {
    pub list: Vec<Agent>,
}

impl AgentList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn add(&mut self, agent: Agent) {
        self.list.push(agent);
        self.sort();
    }

    /// `AgentList::sort` (`AgentList.cpp:36-46`).
    pub fn sort(&mut self) {
        self.list.sort_by(|a, b| {
            a.beat_interval
                .partial_cmp(&b.beat_interval)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id_number.cmp(&b.id_number))
        });
    }

    /// `AgentList::removeDuplicates` (`AgentList.cpp:54-85`).
    fn remove_duplicates(&mut self) {
        self.sort();
        let len = self.list.len();
        for i in 0..len {
            if self.list[i].phase_score < 0.0 {
                continue;
            }
            for j in (i + 1)..len {
                if self.list[j].beat_interval - self.list[i].beat_interval > DEFAULT_BI {
                    break;
                }
                if (self.list[i].beat_time - self.list[j].beat_time).abs() > DEFAULT_BT {
                    continue;
                }
                if self.list[i].phase_score < self.list[j].phase_score {
                    self.list[i].phase_score = -1.0;
                    break;
                }
                self.list[j].phase_score = -1.0;
            }
        }
        self.list.retain(|agent| agent.phase_score >= 0.0);
    }

    /// `AgentList::beatTrack` (`AgentList.cpp:87-131`).
    pub fn beat_track(&mut self, events: &[Event], params: AgentParameters, stop: f64) {
        let phase_given = !self.is_empty() && self.list[0].beat_time >= 0.0;
        for event in events {
            if stop > 0.0 && event.time > stop {
                break;
            }
            let mut created = phase_given;
            let mut prev_beat_interval = -1.0;
            // Le C++ parcourt une copie de la liste pour pouvoir ajouter des
            // agents pendant le parcours ; Rust fait de même avec `mem::take`.
            let current = std::mem::take(&mut self.list);
            for mut agent in current {
                if agent.beat_interval != prev_beat_interval {
                    if prev_beat_interval >= 0.0 && !created && event.time < 5.0 {
                        let mut new_agent = Agent::new(params, prev_beat_interval);
                        new_agent.consider_as_beat(*event, self);
                        self.add(new_agent);
                    }
                    prev_beat_interval = agent.beat_interval;
                    created = phase_given;
                }
                if agent.consider_as_beat(*event, self) {
                    created = true;
                }
                self.add(agent);
            }
            self.remove_duplicates();
        }
    }

    /// `AgentList::bestAgent` (`AgentList.cpp:133-146`).
    pub fn best_agent(&self) -> Option<&Agent> {
        let mut best: Option<&Agent> = None;
        let mut best_score = -1.0;
        for agent in &self.list {
            if agent.events.is_empty() {
                continue;
            }
            // `AgentList::useAverageSalience` vaut false par défaut
            // (`AgentList.cpp:21`) : le score est la somme des saillances.
            let confidence = agent.phase_score;
            if confidence > best_score {
                best = Some(agent);
                best_score = confidence;
            }
        }
        best
    }
}
