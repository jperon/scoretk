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

//! Erreur publique de la façade.

use crate::format::{Format, Sens};

/// Toutes les erreurs pouvant survenir dans la façade.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Erreur du convertisseur MuseScore → modèle.
    #[error(transparent)]
    Mscz2xml(#[from] mscz2xml::Error),

    /// Le format demandé n'a pas encore de convertisseur dans ce sens.
    ///
    /// C'est l'erreur explicite des formats de la feuille de route qui ne sont
    /// pas encore branchés (lecture MusicXML, écriture MIDI, LilyPond, ABC…) :
    /// la façade ne devine pas et ne produit pas de sortie vide.
    #[error("format « {format} » non pris en charge en {sens}")]
    FormatNonPrisEnCharge {
        /// Format demandé.
        format: Format,
        /// Sens demandé (lecture ou écriture).
        sens: Sens,
    },

    /// Le nom de format passé à [`Format::from_str`](std::str::FromStr) est
    /// inconnu.
    ///
    /// [`Format::from_str`]: std::str::FromStr::from_str
    #[error("format inconnu : « {0} » (attendu mscz, mscx, musicxml, mxl, ly, abc ou midi)")]
    FormatInconnu(String),

    /// Le contenu ne correspond à aucun format reconnu.
    #[error("format de fichier non reconnu d'après le contenu")]
    FormatIndetermine,

    /// L'archive ZIP (`.mscz` ou `.mxl`) est illisible.
    #[error("archive ZIP invalide : {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Erreur renvoyée par la crate de lecture MusicXML (feature
    /// `musicxml-import`).
    #[error("lecture MusicXML : {0}")]
    MusicXmlImport(String),

    /// Erreur renvoyée par la crate d'écriture MIDI (feature `midi`).
    #[error("écriture MIDI : {0}")]
    Midi(String),

    /// Erreur renvoyée par la crate d'écriture MuseScore (feature `mscx`).
    #[error("écriture MuseScore : {0}")]
    Mscx(String),
}
