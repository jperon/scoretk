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

//! Couches que MusicXML 4.0 peut porter (§5.3).
//!
//! Source : `docs/agents/format-ref/musicxml.md` « Document shape »,
//! « Notes, rests, chords, beams », « Ties, slurs, tuplets, grace »,
//! « Repeats, jumps, directions, lyrics » et
//! `docs/agents/format-mapping/musicxml.md` lignes 16–47 : L0–L5 (pas de L6).
//! La déclaration décrit le **format**, pas ce que l'écrivain produit
//! aujourd'hui (les positions `<default-x/y>` et `<play>` restent à couvrir).

use scoretk_core::model::{Capabilities, Layers};

/// Couches écrivables vers MusicXML.
pub const CAPABILITIES: Capabilities = Capabilities::with_kinds(
    Layers::NONE,
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK)
        .union(Layers::ENGRAVING),
    scoretk_xml_common::XML_ITEM_KINDS,
);
