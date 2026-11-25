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

//! Couches que le Standard MIDI File peut porter (§5.3).
//!
//! Source : `docs/agents/format-ref/midi.md` « Events and time » (L0, L4),
//! « Meta-event types » `FF 58`/`FF 59` (L1), `FF 51` (L4), `FF 03`–`FF 07`
//! et « Lyrics: RP-017 and RP-026 » (L3), « Instruments, parts, repeats »
//! (L0/L1/L4), et `docs/agents/format-mapping/midi.md` lignes 15–36. Le SMF
//! n'a ni mesures explicites, ni tuplets, ni gravure : L5 et L6 ne sont pas
//! déclarés ; L1 est déclaré pour les méta-événements de mesure, de tonalité
//! et de marqueur. La déclaration décrit le **format**, pas le lecteur actuel.

use scoretk_core::model::{Capabilities, Layers};

/// Couches lisibles depuis un SMF.
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK),
    Layers::NONE,
);
