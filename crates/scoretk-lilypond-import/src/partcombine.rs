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

//! Règle `unisono` de `\partCombine`, portée de `scm/part-combiner.scm`.
//!
//! `determine-split-list` (l. 339-360) compare les notes des deux parties à
//! chaque instant ; `chord-range` (l. 436-480) place les hauteurs égales dans
//! l'état `chords`, puis l'état `unisono` (l. 550-566) n'émet qu'une attaque.
//! Appliquée après interprétation, aux seules notes principales : les grâces
//! et les durées mises à l'échelle ont un moment différent pour le combiner
//! et sont laissées telles quelles.

use scoretk_core::model::Fraction;

use crate::events::{ChordContent, EventKind, InterpretedVoice, VoiceEvent};

/// Empreinte d'une note principale jouée : attaque, durée réelle, hauteur MIDI.
fn clef(event: &VoiceEvent) -> Option<(Fraction, Fraction, u8)> {
    match &event.kind {
        EventKind::Chord(chord) => {
            if !chord.grace_before.is_empty() || !chord.grace_after.is_empty() {
                return None;
            }
            match &chord.content {
                ChordContent::Notes(notes) if notes.len() == 1 => {
                    Some((event.tick, chord.actual, notes[0].pitch))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Retire de la seconde voix les notes déjà jouées par la première aux
/// mêmes attaque, durée et hauteur (état `unisono` de part-combiner.scm).
pub(crate) fn retire_unissons(first: &InterpretedVoice, second: &mut InterpretedVoice) {
    let refus: Vec<(Fraction, Fraction, u8)> = first.events.iter().filter_map(clef).collect();
    second
        .events
        .retain(|event| clef(event).is_none_or(|c| !refus.contains(&c)));
}
