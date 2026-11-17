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

use mscz2xml::ExportOptions;

/// Options transmises à [`write`](crate::write).
///
/// Les valeurs par défaut sont celles de l'export MusicXML de `mscz2xml` :
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
}

impl Default for Options {
    fn default() -> Self {
        let export = ExportOptions::default();
        Self {
            software: export.software,
            encoding_date: export.encoding_date,
            concert_pitch: export.concert_pitch,
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
        }
    }
}
