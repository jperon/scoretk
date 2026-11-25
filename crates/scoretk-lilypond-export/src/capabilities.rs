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

//! Couches que LilyPond 2.26 peut porter (§5.3).
//!
//! Source : `docs/agents/format-ref/lilypond.md` « Durations, ticks, bars »,
//! « Ties, slurs, tuplets, grace », « Repeats and lyrics »,
//! articulations/ornaments (§3.1–3.3), `\paper`/`\layout` (L5) et le second
//! `\score` MIDI (L4) ; `format-mapping/lilypond.md` lignes 16–40 : L0–L5,
//! pas de L6. La déclaration décrit le **format**, pas ce que l'écrivain
//! produit aujourd'hui (`\layout {}` est vide, le figured bass est omis).

use scoretk_core::model::{Capabilities, Layers};

/// Couches écrivables vers LilyPond.
pub const CAPABILITIES: Capabilities = Capabilities::with_kinds(
    Layers::NONE,
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK)
        .union(Layers::ENGRAVING),
    scoretk_lilypond_common::LY_ITEM_KINDS,
);
