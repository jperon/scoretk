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

//! **scoretk-mscx** : écriture MuseScore (`.mscx`, `.mscz`) depuis le modèle
//! pivot de `mscz2xml`.
//!
//! L'écrivain est le portage de `engraving/rw/write/` de MuseScore Studio
//! (`Writer::writeScore`, `StaffWrite`, `MeasureWrite`, `TWrite`,
//! `ConnectorInfoWriter`) et de `engraving/infrastructure/mscwriter.cpp` pour
//! le conteneur. Il produit un fichier `<museScore version="4.70">` que le
//! lecteur `mscz2xml::read` relit à l'identique : c'est la mesure de fidélité
//! du test d'aller-retour (`tests/roundtrip.rs`).
//!
//! La bibliothèque reste portable : tout se passe en mémoire, sans disque,
//! sans thread et sans horloge.

mod container;
mod error;
mod names;
mod text;
mod write;
mod xmlwriter;

pub use error::Error;
pub use write::writer::{MSC_VERSION, PROGRAM_VERSION};

/// Résultat de l'écriture.
pub type Result<T> = std::result::Result<T, Error>;

/// Écrit une partition au format `.mscx` (XML MuseScore 4.x).
///
/// Le document produit est déterministe : il ne dépend que du modèle. Il
/// s'ouvre par `<?xml version="1.0" encoding="UTF-8"?>` et porte
/// `<museScore version="4.70">`.
pub fn write_mscx(score: &mscz2xml::model::Score) -> Result<String> {
    Ok(write::writer::write_score(score))
}

/// Écrit une partition au format `.mscz` (archive ZIP de MuseScore 4.x).
///
/// L'archive contient `META-INF/container.xml`, `score.mscx`,
/// `score_style.mss`, `audiosettings.json` et `viewsettings.json` : c'est ce
/// que `MscSaver::writeMscz` écrit hors vignette et médias, que la
/// bibliothèque ne fabrique pas.
pub fn write_mscz(score: &mscz2xml::model::Score) -> Result<Vec<u8>> {
    let mscx = write::writer::write_score(score);
    container::write_container(score, &mscx)
}
