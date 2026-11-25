// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiexporter.cpp` et
// `meiconverter.cpp` de MuseScore Studio 4.7.4, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Écriture de MEI 5.1 (`mei-basic`) depuis le modèle pivot.

mod capabilities;
mod header;
mod writer;

pub use capabilities::CAPABILITIES;
pub use scoretk_mei_common::Error;

use scoretk_core::model::Score;

/// Options d'écriture MEI, calquées sur `MeiConfiguration`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeiOptions {
    /// Écrit les ruptures de page et de système (`layout`).
    pub layout: bool,
    /// Date d'encodage écrite dans le `<date isodate>`, constante par défaut
    /// pour la reproductibilité.
    pub encoding_date: String,
}

impl Default for MeiOptions {
    fn default() -> Self {
        Self {
            layout: true,
            encoding_date: "2026-01-01T00:00:00".to_owned(),
        }
    }
}

/// Écrit une partition au format MEI 5.1.
pub fn write_mei(score: &Score) -> Result<String, Error> {
    write_mei_with(score, &MeiOptions::default())
}

/// Écrit une partition avec des options explicites.
pub fn write_mei_with(score: &Score, options: &MeiOptions) -> Result<String, Error> {
    writer::write(score, options)
}
