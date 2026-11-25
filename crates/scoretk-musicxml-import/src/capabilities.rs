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
//! Source : `docs/agents/format-ref/musicxml.md` « Document shape » (L3/L5),
//! « Pitch, accidentals, key », « Clef, time, staves, parts »,
//! « Notes, rests, chords, beams », « Ties, slurs, tuplets, grace »,
//! « Repeats, jumps, directions, lyrics » (L1/L2/L4) et
//! `docs/agents/format-mapping/musicxml.md` lignes 16–47. Les `<defaults>`,
//! `<credit>` et `<beam>`/`<stem>` justifient L5 ; `<play>`, `<sound>` et
//! `<midi-instrument>` justifient L4. MusicXML n'a pas de charge utile opaque
//! réservée à un seul lecteur : L6 n'est pas déclaré. La déclaration décrit
//! le **format**, pas l'état actuel du lecteur.

use scoretk_core::model::{Capabilities, Layers};

/// Couches lisibles depuis MusicXML.
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK)
        .union(Layers::ENGRAVING),
    Layers::NONE,
);
