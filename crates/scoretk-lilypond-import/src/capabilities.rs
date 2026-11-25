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
//! « Pitch, key, clef, accidentals », « Parts, staves, voices, chords »,
//! « Ties, slurs, tuplets, grace », articulations/ornaments (§3.1–3.3),
//! « Repeats and lyrics » et « \paper, \layout » pour L5 ; la seconde
//! partition MIDI `\score` porte L4 (`format-mapping/lilypond.md` ligne 38).
//! Voir aussi `format-mapping/lilypond.md` lignes 16–40 : L0–L5, pas de L6.
//! La déclaration décrit le **format**, pas le sous-ensemble lu aujourd'hui.

use scoretk_core::model::{Capabilities, Layers};

/// Couches lisibles depuis LilyPond.
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK)
        .union(Layers::ENGRAVING),
    Layers::NONE,
);
