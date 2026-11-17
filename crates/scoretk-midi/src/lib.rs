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
//! `engraving/compat/midi/`) : le modèle de [`mscz2xml`] est rendu en
//! performance, puis écrit en Standard MIDI File format 1 (une piste par
//! portée, division [`repeat_list::DIVISION`] unités par noire).
//!
//! La bibliothèque reste portable : aucun accès au disque, aucun thread, aucune
//! horloge ; la sortie est un [`Vec<u8>`] en mémoire, donc le crate compile pour
//! `wasm32-unknown-unknown`.
//!
//! ```
//! use mscz2xml::model::Score;
//! use scoretk_midi::write_midi;
//!
//! let score = Score::new();
//! let bytes = write_midi(&score)?;
//! assert_eq!(&bytes[..4], b"MThd");
//! # Ok::<(), scoretk_midi::Error>(())
//! ```

mod error;
pub mod import;
mod pause_map;
mod render;
pub mod repeat_list;
mod velocity;
mod writer;

pub use error::Error;
pub use import::{read_midi, read_midi_with};
pub use repeat_list::{RepeatList, RepeatSegment, DIVISION, UNITS_PER_WHOLE};

use mscz2xml::model::Score;

/// Options d'écriture MIDI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiOptions {
    /// Déroule les reprises, voltas et sauts (`RepeatList::unwind`). Vrai par
    /// défaut, comme l'option `io/midi/expandRepeats` de MuseScore.
    pub expand_repeats: bool,
}

impl Default for MidiOptions {
    fn default() -> Self {
        Self {
            expand_repeats: true,
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
pub fn write_midi(score: &Score) -> Result<Vec<u8>, Error> {
    write_midi_with(score, MidiOptions::default())
}

/// Écrit une partition en MIDI avec des options explicites.
pub fn write_midi_with(score: &Score, options: MidiOptions) -> Result<Vec<u8>, Error> {
    let repeat_list = if options.expand_repeats {
        RepeatList::unwind(score)
    } else {
        RepeatList::flatten(score)
    };
    let performance = render::render(score, &repeat_list)?;
    writer::write_smf(&performance)
}
