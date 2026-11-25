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
//!
//! La couche L1 est trop grossière pour le SMF : `[SMF §Container layout]`
//! « no bar/measure/staff/voice fields » et `[SMF §Instruments, parts,
//! repeats]` « no repeats, voltas, tuplets, ties or slurs ». Seuls les types
//! `time-sig` (`FF 58`) et `key-sig` (`FF 59`) sont portés (§"Meta-event
//! types"). Les autres types de L1 (mesure, barre, clef, reprise, tuplet,
//! `marker` de reprise…) n'ont aucun objet SMF correspondant et ne sont
//! donc pas déclarés, conformément à la règle de `model-modularisation.md`
//! §5.3 (la déclaration est celle du format, pas une cible de score).
//! `FF 06`/`FF 07` sont des **repères de répétition** (« rehearsal mark /
//! section name »), pas des marqueurs de reprise : ils portent le type L3
//! `rehearsal-mark`. De même L2 ne retient que `note` et `rest` (silence entre
//! deux `Note On`, reconstruit par l'importeur), L3 que `lyric` (`FF 05`,
//! RP-017), `staff-text`/`system-text` (`FF 01`, « Text event »),
//! `rehearsal-mark` (`FF 06`/`FF 07`), `tempo` (`FF 51`) et `title`
//! (`FF 02`/`FF 03`/`FF 04`, RP-026 `{#TITLE=}`), et L4 les champs réellement
//! encodés (programme, canal, volume `CC 7`, panoramique `CC 10`, vélocité).
//! Les symboles dynamiques, la gravure et les tables d'articulation
//! n'existent pas dans un SMF.

use scoretk_core::model::{Capabilities, Layers};

/// Types d'items qu'un SMF peut porter (citations ci-dessus).
pub const SMF_KINDS: &[&str] = &[
    "time-sig",
    "key-sig",
    "rehearsal-mark",
    "note",
    "rest",
    "lyric",
    "staff-text",
    "system-text",
    "tempo",
    "title",
    "instrument",
    "program",
    "channel",
    "volume",
    "pan",
    "velocity",
];

/// Couches lisibles depuis un SMF.
pub const CAPABILITIES: Capabilities = Capabilities::with_kinds(
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::PLAYBACK),
    Layers::NONE,
    SMF_KINDS,
);
