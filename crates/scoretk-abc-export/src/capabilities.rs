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

//! Couches que ABC 2.1 peut porter (§5.3).
//!
//! Source : `docs/agents/format-ref/abc.md` §3.1, §4.3–4.5, §4.8–4.17, §5,
//! §6.2 (`%%MIDI`) et §7 ; `format-mapping/abc.md` lignes 11–33 : L0–L4.
//! L5 n'est pas déclaré (aucune position ni style de page dans ABC 2.1,
//! ligatures implicites). La déclaration décrit le **format**, pas ce que
//! l'écrivain produit aujourd'hui (les instruments et le style sont omis).

use scoretk_core::model::{Capabilities, Layers};

/// Couches écrivables vers ABC.
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::NONE,
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK),
);
