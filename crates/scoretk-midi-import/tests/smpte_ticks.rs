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

//! Ticks SMPTE : conversion fidèle à `toMuseScoreTicks`.

use scoretk_core::model::Fraction;
use scoretk_midi_import::midifile::to_muse_score_ticks;

/// `toMuseScoreTicks` (`importmidi_fraction.cpp:350-354`) renvoie
/// `ReducedFraction::fromTicks(tick)` tel quel en division SMPTE : le convertir
/// en noires à 120 BPM allonge/accourcit artificiellement les partitions dont
/// l'en-tête utilise une division temporelle (`division-ref.mscx`, 2.x).
#[test]
fn ticks_smpte_restent_bruts() {
    let raw = to_muse_score_ticks(1000, 80, true);
    assert_eq!(raw, Fraction::from_ticks(1000, 1920));
    let zero = to_muse_score_ticks(0, 80, true);
    assert_eq!(zero, Fraction::ZERO);

    // Cas non concerné : la division métrique garde son arrondi au demi-tick
    // (`importmidi_fraction.cpp:361-378`).
    let metric = to_muse_score_ticks(480, 480, false);
    assert_eq!(metric, Fraction::from_ticks(480, 1920));
}
