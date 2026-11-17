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

//! Options d'écriture communes à tous les formats.

use scoretk_core::ExportOptions;

/// Options d'écriture MIDI.
///
/// Reprend les réglages utiles de [`scoretk_midi::MidiOptions`] sans exposer
/// la crate optionnelle : la façade reste utilisable avec les features
/// désactivées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiOptions {
    /// Déroule les reprises, voltas et sauts dans la sortie MIDI. Vrai par
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

/// Options transmises à [`write`](crate::write).
///
/// Les valeurs par défaut sont celles de l'export MusicXML de `scoretk_core` :
/// logiciel annoncé et date d'encodage constantes (sortie reproductible). La
/// CLI substitue la date du jour à `encoding_date` si l'utilisateur ne la fige
/// pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Contenu de `<software>` (MusicXML).
    pub software: String,
    /// Contenu de `<encoding-date>` (MusicXML, `AAAA-MM-JJ`).
    pub encoding_date: String,
    /// Exporter les hauteurs en sons réels (MusicXML).
    pub concert_pitch: bool,
    /// Réglages de la sortie MIDI (reprises).
    pub midi: MidiOptions,
}

impl Default for Options {
    fn default() -> Self {
        let export = ExportOptions::default();
        Self {
            software: export.software,
            encoding_date: export.encoding_date,
            concert_pitch: export.concert_pitch,
            midi: MidiOptions::default(),
        }
    }
}

impl Options {
    /// Convertit vers les options de l'export MusicXML.
    pub(crate) fn export_options(&self) -> ExportOptions {
        ExportOptions {
            software: self.software.clone(),
            encoding_date: self.encoding_date.clone(),
            concert_pitch: self.concert_pitch,
            // L'export MusicXML ordinaire ne pose aucun identifiant : les
            // références MuseScore 4.7.4 n'en contiennent pas. Seul
            // `write_playback` force `stable_ids`.
            stable_ids: false,
        }
    }
}
