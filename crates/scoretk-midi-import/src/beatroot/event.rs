// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Event.h` du plug-in BeatRoot (Vamp, Queen Mary University of
// London), copyright 2011 Simon Dixon, Chris Cannam et QMUL, licence GPL-2.0-
// ou-ultérieure, modifié (réécrit en Rust) en 2026. Référence :
// `src/importexport/midi/thirdparty/beatroot/Event.h`.
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

//! Une attaque : temps en secondes et saillance.

/// `Event` (`Event.h:21-37`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub time: f64,
    pub salience: f64,
}

impl Event {
    pub const fn new(time: f64, salience: f64) -> Self {
        Self { time, salience }
    }
}
