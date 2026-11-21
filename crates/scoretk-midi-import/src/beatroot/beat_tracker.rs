// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `BeatTracker.h` / `BeatTracker.cpp` du plug-in BeatRoot (Vamp,
// Queen Mary University of London), copyright 2011 Simon Dixon, Chris Cannam
// et QMUL, licence GPL-2.0-ou-ultérieure, modifié (réécrit en Rust) en 2026.
// Référence : `src/importexport/midi/thirdparty/beatroot/BeatTracker.{h,cpp}`.
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

//! Point d'entrée `beatTrack` (`BeatTracker.cpp`).

use super::agent::{Agent, AgentParameters};
use super::agent_list::AgentList;
use super::event::Event;
use super::induction::beat_induction;

/// `BeatTracker::beatTrack` (`BeatTracker.cpp:24-67`).
///
/// Sans temps imposés, les hypothèses viennent de l'induction ; avec un tempo
/// connu, un seul agent est créé avec ce tempo.
pub fn beat_track_with(events: &[Event], params: AgentParameters, beats: &[Event]) -> Vec<f64> {
    let mut agents = if beats.len() >= 2 {
        let count = beats.len() - 1;
        let ioi = (beats[beats.len() - 1].time - beats[0].time) / count as f64;
        let mut list = AgentList::new();
        list.list.push(Agent::new(params, ioi));
        list
    } else {
        beat_induction(params, events)
    };
    if !beats.is_empty() {
        let beat_time = beats[beats.len() - 1].time;
        let count = (beats.len() - 1) as i32;
        for agent in &mut agents.list {
            agent.beat_time = beat_time;
            agent.beat_count = count;
            agent.events = beats.to_vec();
        }
    }
    agents.beat_track(events, params, -1.0);
    let Some(best) = agents.best_agent() else {
        return Vec::new();
    };
    let end = if beats.is_empty() {
        -1.0
    } else {
        beats[beats.len() - 1].time
    };
    let mut agent = best.clone();
    agent.fill_beats(end);
    agent.events.into_iter().map(|event| event.time).collect()
}

/// `beatTrack(const EventList&)` (`BeatTracker.cpp:75-77`).
pub fn beat_track(events: &[Event]) -> Vec<f64> {
    beat_track_with(events, AgentParameters::default(), &[])
}
