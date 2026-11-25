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
///
/// Les variantes qui transportent directement l'erreur d'un crate de sens
/// n'existent que si la feature correspondante est active.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Erreur du convertisseur MuseScore → modèle (feature `musescore-import`).
    #[cfg(feature = "musescore-import")]
    #[error(transparent)]
    MuseScoreImport(#[from] scoretk_musescore_import::Error),

    /// Erreur de l'export MusicXML (feature `musicxml-export`).
    #[cfg(feature = "musicxml-export")]
    #[error(transparent)]
    MusicXmlExport(#[from] scoretk_musicxml_export::Error),

    /// Le format demandé n'a pas de convertisseur dans ce sens.
    ///
    /// C'est l'erreur explicite d'un format dont la feature de ce sens est
    /// désactivée, ou qui n'est pas branché : la façade ne devine pas et ne
    /// produit pas de sortie vide.
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
    #[error("format inconnu : « {0} » (attendu mscz, mscx, musicxml, mxl, mei, ly, abc ou midi)")]
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

    /// Erreur renvoyée par la crate d'écriture MIDI (feature `midi-export`).
    #[error("écriture MIDI : {0}")]
    Midi(String),

    /// Erreur renvoyée par la crate de lecture MIDI (feature `midi-import`).
    #[error("lecture MIDI : {0}")]
    MidiImport(String),

    /// Erreur renvoyée par les crates LilyPond (features `lilypond-import` et
    /// `lilypond-export`).
    #[error("LilyPond : {0}")]
    LilyPond(String),

    /// Erreur renvoyée par la crate d'écriture MuseScore (feature
    /// `musescore-export`).
    #[error("écriture MuseScore : {0}")]
    Mscx(String),

    /// Erreur renvoyée par les crates ABC (features `abc-import` et
    /// `abc-export`).
    #[error("ABC : {0}")]
    Abc(String),

    /// Erreur renvoyée par les crates MEI (features `mei-import` et
    /// `mei-export`).
    #[error("MEI : {0}")]
    Mei(String),
}

impl Error {
    /// Vrai si l'échec tient à une entrée **délibérément non prise en charge**,
    /// et non à un défaut de lecture.
    ///
    /// Qualifient :
    ///
    /// - `scoretk_musescore_import::Error::UnsupportedVersion` : version
    ///   MuseScore hors du sous-ensemble porté (aujourd'hui < 1.15, `read114`
    ///   n'est pas porté) ;
    /// - [`Error::FormatNonPrisEnCharge`] : format sans convertisseur dans le
    ///   sens demandé (feature du sens désactivée).
    ///
    /// Toute autre erreur — XML mal formé, ZIP invalide, source illisible —
    /// reste un échec ordinaire : la distinction sert à ne pas imputer à
    /// scoretk ce qu'il refuse par construction, jamais à masquer une
    /// régression de lecture. Les lecteurs MusicXML, LilyPond, ABC et MIDI
    /// n'ont pas, aujourd'hui, de variante « version non prise en charge » ;
    /// leurs erreurs arrivent ici sous forme de texte (`MusicXmlImport`,
    /// `LilyPond`, `Abc`, `MidiImport`) et restent donc des échecs ordinaires.
    #[must_use]
    pub fn is_unsupported(&self) -> bool {
        match self {
            #[cfg(feature = "musescore-import")]
            Self::MuseScoreImport(scoretk_musescore_import::Error::UnsupportedVersion(_)) => true,
            Self::FormatNonPrisEnCharge { .. } => true,
            _ => false,
        }
    }
}
