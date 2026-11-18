// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/midi/internal/midiexport/` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Erreurs partagées par la lecture et l'écriture MIDI.

use thiserror::Error;

/// Erreur de traitement MIDI (lecture ou écriture).
#[derive(Debug, Error)]
pub enum Error {
    /// La résolution de la partition est invalide.
    #[error("division MIDI invalide : {0}")]
    InvalidDivision(u32),
    /// Un tick calculé sort des bornes d'un entier signé 32 bits (le SMF
    /// encode les deltas en 28 bits utiles).
    #[error("tick hors bornes MIDI : {0}")]
    TickOverflow(i64),
    /// Une valeur ne tient pas dans le codage MIDI (canal, note, vélocité).
    #[error("valeur MIDI hors bornes : {0}")]
    ValueOutOfRange(i64),
    /// L'encodage du fichier a échoué.
    #[error("écriture du fichier MIDI impossible : {0}")]
    Write(&'static str),
    /// Le fichier MIDI est illisible (octets SMF invalides).
    #[error("fichier MIDI invalide : {0}")]
    MidiParse(String),
}

/// Résultat de l'écriture MIDI.
pub type Result<T> = std::result::Result<T, Error>;
