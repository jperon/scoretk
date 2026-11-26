// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Quote_iterator::process` de `lily/quote-iterator.cc` (LilyPond,
// Copyright (C) 1998--2026 Han-Wen Nienhuys et al.), lignes 225-275 : une
// citation transpose ses événements de l'intervalle entre la transposition
// d'instrument de la cible et celle de la source, réécrit en Rust en 2026.
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

//! Écart de transposition d'une citation (`\quoteDuring`, `\cueDuring`).
//!
//! `lily/quote-iterator.cc:229-271` lit `quoted-transposition` sur la
//! citation, sinon `instrumentTransposition` du contexte cible, et transpose
//! chaque événement cité de `pitch_interval (me, quote)`. La transposition
//! cible est déjà appliquée par le constructeur à toutes les notes de la
//! portée ; il suffit donc d'ajouter aux hauteurs *écrites* citées
//! l'écart `source − cible` avant cette application.

use crate::ast::{Context, Music, PitchSpec};

/// Écart (degrés, demi-tons) à appliquer aux hauteurs écrites d'une citation.
///
/// `source` est le `\transposition` porté par la musique citée, `cible` le
/// dernier `\transposition` vu dans la voix qui reçoit la citation. Sans
/// transposition source, LilyPond ne transpose pas les événements cités.
pub(crate) fn quote_delta(source: &Music, target: Option<&PitchSpec>) -> (i32, i32) {
    let Some(source) = cited_transposition(source) else {
        return (0, 0);
    };
    let (source_diatonic, source_chromatic) = crate::pitch::instrument_transpose(&source);
    let (target_diatonic, target_chromatic) = target
        .map(crate::pitch::instrument_transpose)
        .unwrap_or((0, 0));
    (
        source_diatonic - target_diatonic,
        source_chromatic - target_chromatic,
    )
}

/// Premier `\transposition` porté par une musique citée.
fn cited_transposition(music: &Music) -> Option<PitchSpec> {
    match music {
        Music::Context(Context::InstrumentTranspose(pitch)) => Some(*pitch),
        Music::Seq(items) | Music::Sim(items) | Music::SimVoices(items) => {
            items.iter().find_map(cited_transposition)
        }
        Music::Tag { music, .. } | Music::Filtered { music, .. } => cited_transposition(music),
        Music::New(group) => group.staves.iter().find_map(|staff| {
            staff
                .voices
                .iter()
                .find_map(|voice| cited_transposition(&voice.music))
        }),
        Music::Context(context) => match context {
            Context::Transpose { music, .. }
            | Context::Harmonic { music, .. }
            | Context::Tuplet { music, .. }
            | Context::Grace { music, .. }
            | Context::Repeat { body: music, .. }
            | Context::Cue { music, .. }
            | Context::Invisible { music }
            | Context::Retrograde { music }
            | Context::NonRelative { music } => cited_transposition(music),
            Context::AfterGrace { main, grace, .. } => {
                cited_transposition(main).or_else(|| cited_transposition(grace))
            }
            Context::Alternative(items) => items.iter().find_map(cited_transposition),
            Context::Volta { music, .. } => cited_transposition(music),
            _ => None,
        },
        _ => None,
    }
}
