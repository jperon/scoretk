// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage du plug-in BeatRoot (`src/importexport/midi/thirdparty/beatroot/`)
// de MuseScore Studio, Copyright (C) 2011 Simon Dixon, Chris Cannam et QMUL,
// licence GPL-2.0-ou-ultérieure, modifié (réécrit en Rust) en 2026. Voir
// NOTICE. Un module Rust par fichier C++ d'origine.
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

//! Suiveur de temps BeatRoot, utilisé par `MidiBeat::findBeatLocations`.

pub mod agent;
pub mod agent_list;
pub mod beat_tracker;
pub mod event;
pub mod induction;

pub use agent::{Agent, AgentParameters};
pub use agent_list::AgentList;
pub use beat_tracker::{beat_track, beat_track_with};
pub use event::Event;
