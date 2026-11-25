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
//! Source : `docs/agents/format-ref/abc.md` §3.1 (en-têtes), §4.3–4.5
//! (durées/reprises), §4.8–4.10 (barres et voltas), §4.11–4.17
//! (liaisons, agréments, tuplets, accords), §5 (paroles), §6.2 (`%%MIDI`,
//! L4), §7 (voix) et `format-mapping/abc.md` lignes 11–33. L5 n'est **pas**
//! déclaré : ABC 2.1 n'a ni style de page, ni positions d'éléments, et ses
//! ligatures sont implicites (§4.7, « Model gaps » n°4 du mapping). La
//! déclaration décrit le **format**, pas le sous-ensemble lu aujourd'hui.

use scoretk_core::model::{Capabilities, Layers};

/// Couches lisibles depuis ABC.
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK),
    Layers::NONE,
);
