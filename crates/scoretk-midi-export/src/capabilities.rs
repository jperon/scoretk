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
//! Source : `docs/agents/format-ref/midi.md` « Events and time », « Meta-event
//! types » (`FF 51`, `FF 58`, `FF 59`, textes), « Instruments, parts, repeats »
//! et « Lyrics: RP-017 and RP-026 » ; `docs/agents/format-mapping/midi.md`
//! lignes 15–36 : L0–L4, pas de L5/L6. La déclaration décrit le **format**,
//! pas ce que l'écrivain émet aujourd'hui (les marqueurs `FF 06` manquent).
//!
//! Comme pour la lecture, la granularité par type d'item est nécessaire :
//! `[SMF §Container layout]` exclut mesure/barre/clef, `[SMF §Instruments,
//! parts, repeats]` exclut reprises/tuplets/liaisons et `[SMF §Meta-event
//! types]` ne porte que `FF 58`/`FF 59`/`FF 06` en L1, `FF 05`/`FF 51`/
//! `FF 03`–`FF 04` en L3 et les messages de canal en L4. Voir
//! `scoretk-midi-import/src/capabilities.rs` pour la liste et les citations.

use scoretk_core::model::{Capabilities, Layers};

/// Types d'items qu'un SMF peut porter (citations ci-dessus ; la liste est
/// identique à celle de `scoretk-midi-import/src/capabilities.rs`).
pub const SMF_KINDS: &[&str] = &[
    "time-sig",
    "key-sig",
    "marker",
    "note",
    "rest",
    "lyric",
    "tempo",
    "title",
    "instrument",
    "program",
    "channel",
    "volume",
    "pan",
    "velocity",
];

/// Couches écrivables vers un SMF.
pub const CAPABILITIES: Capabilities = Capabilities::with_kinds(
    Layers::NONE,
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK),
    SMF_KINDS,
);
