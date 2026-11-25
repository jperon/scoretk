// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/midi/internal/midiexport/exportmidi.cpp` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026.
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

//! Écriture MIDI de `scoretk`.
//!
//! Portage de l'export MIDI de MuseScore Studio
//! (`importexport/midi/internal/midiexport/exportmidi.cpp`,
//! `engraving/compat/midi/`) : le modèle de [`scoretk_core`] est rendu en
//! performance, puis écrit en Standard MIDI File format 1 (une piste par
//! portée, division [`repeat_list::DIVISION`] unités par noire).
//!
//! La bibliothèque reste portable : aucun accès au disque, aucun thread, aucune
//! horloge ; la sortie est un [`Vec<u8>`] en mémoire, donc le crate compile pour
//! `wasm32-unknown-unknown`.
//!
//! ```
//! use scoretk_core::model::Score;
//! use scoretk_midi_export::write_midi;
//!
//! let score = Score::new();
//! let bytes = write_midi(&score)?;
//! assert_eq!(&bytes[..4], b"MThd");
//! # Ok::<(), scoretk_midi_export::Error>(())
//! ```

// Les erreurs sont partagées avec la lecture MIDI (types neutres du format).
pub use scoretk_midi_common as error;
pub mod capabilities;
mod fixup;
mod glissando;

pub use capabilities::{CAPABILITIES, SMF_KINDS};
mod harmony;
mod ornaments;
mod pause_map;
mod render;
pub mod repeat_list;
mod smf_text;
pub mod timemap;
mod velocity;
mod writer;

pub use error::{Error, Result};
pub use repeat_list::{RepeatList, RepeatSegment, DIVISION, UNITS_PER_WHOLE};
pub use timemap::{MeasureSync, NoteSync, PauseSync, TempoChange, Timemap};

use scoretk_core::model::Score;

/// Options d'écriture MIDI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiOptions {
    /// Déroule les reprises, voltas et sauts (`RepeatList::unwind`). Vrai par
    /// défaut, comme l'option `io/midi/expandRepeats` de MuseScore.
    pub expand_repeats: bool,
    /// Extension additive « fidélité SMF » : écrit les textes et repères que
    /// le format porte (`FF 01`, `FF 02`, `FF 06`, `FF 07`) en plus des noms
    /// de piste et des paroles. Faux par défaut, comme MuseScore 4.7.4.
    pub smf_text: bool,
}

impl Default for MidiOptions {
    fn default() -> Self {
        Self {
            expand_repeats: true,
            smf_text: false,
        }
    }
}

/// Écrit une partition en Standard MIDI File format 1.
///
/// Équivalent de `ExportMidi::write` avec les réglages par défaut de
/// MuseScore : reprises déroulées, canal et programme tirés de l'instrument,
/// vélocités des nuances et soufflets, articulations, liaisons de prolongation
/// fusionnées, notes d'ornement.
///
/// # Erreurs
///
/// Renvoie une [`Error`] si la partition ne peut pas être écrite (division
/// invalide, dépassement des bornes MIDI).
pub fn write_midi(score: &Score) -> Result<Vec<u8>> {
    write_midi_with(score, MidiOptions::default())
}

/// Écrit une partition en MIDI avec des options explicites.
pub fn write_midi_with(score: &Score, options: MidiOptions) -> Result<Vec<u8>> {
    write_midi_impl(score, options, false).map(|(bytes, _)| bytes)
}

/// Écrit une partition en MIDI et renvoie la carte des temps de la performance.
///
/// Le fichier produit est identique à l'octet près à celui de [`write_midi`] :
/// la carte est un sous-produit du même rendu.
pub fn write_midi_with_timemap(score: &Score) -> Result<(Vec<u8>, Timemap)> {
    write_midi_with_timemap_and(score, MidiOptions::default())
}

/// Écrit une partition en MIDI avec des options explicites et renvoie la carte
/// des temps.
pub fn write_midi_with_timemap_and(
    score: &Score,
    options: MidiOptions,
) -> Result<(Vec<u8>, Timemap)> {
    write_midi_impl(score, options, true)
}

/// Rend la partition, avec ou sans construction de la carte des temps.
///
/// `capture` faux court-circuite l'enregistrement des mesures et des attaques
/// (voir [`render::render`]) : le MIDI produit est identique à l'octet près.
fn write_midi_impl(
    score: &Score,
    options: MidiOptions,
    capture: bool,
) -> Result<(Vec<u8>, Timemap)> {
    // L'écrivain obéit uniquement à l'appelant : un consommateur qui demande
    // `expandRepeats: true` doit obtenir les reprises dépliées, même pour un
    // `.ly` sans `\unfoldRepeats`. L'oracle LilyPond, lui, joue le fichier tel
    // qu'écrit en passant `--no-expand-repeats` (voir `scripts/oracle/midi`).
    let repeat_list = if options.expand_repeats {
        RepeatList::unwind(score)
    } else {
        RepeatList::flatten(score)
    };
    let rendered = render::render(score, &repeat_list, capture, options.smf_text)?;
    let bytes = writer::write_smf(&rendered.performance)?;
    Ok((bytes, rendered.timemap))
}
