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

//! Points d'accroche des convertisseurs optionnels.
//!
//! Chaque feature de `Cargo.toml` active un convertisseur absent par défaut.
//! Le point de branchement est ici, et nulle part ailleurs ; la procédure
//! complète est dans `docs/agents/scoretk.md` § 3.
//!
//! - feature `musicxml-import` : crate `scoretk-musicxml` (t34), fonction
//!   `read_musicxml(&[u8]) -> Result<Score>` ;
//! - feature `midi` : crate `scoretk-midi` (t35), fonction
//!   `write_midi(&Score) -> Result<Vec<u8>>` ;
//! - feature `lilypond` : crate `scoretk-lilypond` (t36), fonction
//!   `write_lilypond(&Score) -> Result<String>` ;
//! - feature `mscx` : crate `scoretk-mscx` (t38), fonctions
//!   `write_mscx(&Score) -> Result<String>` et
//!   `write_mscz(&Score) -> Result<Vec<u8>>`.

use mscz2xml::model::Score;

use crate::error::Error;
#[cfg(not(all(
    feature = "musicxml-import",
    feature = "midi",
    feature = "lilypond",
    feature = "mscx"
)))]
use crate::format::{Format, Sens};
use crate::Result;

#[cfg(feature = "musicxml-import")]
pub(crate) fn read_musicxml(bytes: &[u8]) -> Result<Score> {
    scoretk_musicxml::read_musicxml(bytes).map_err(|error| Error::MusicXmlImport(error.to_string()))
}

#[cfg(not(feature = "musicxml-import"))]
pub(crate) fn read_musicxml(_bytes: &[u8]) -> Result<Score> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::MusicXml,
        sens: Sens::Lecture,
    })
}

#[cfg(feature = "midi")]
pub(crate) fn write_midi(score: &Score) -> Result<Vec<u8>> {
    scoretk_midi::write_midi(score).map_err(|error| Error::Midi(error.to_string()))
}

#[cfg(not(feature = "midi"))]
pub(crate) fn write_midi(_score: &Score) -> Result<Vec<u8>> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Midi,
        sens: Sens::Ecriture,
    })
}

#[cfg(feature = "lilypond")]
pub(crate) fn write_lilypond(score: &Score) -> Result<Vec<u8>> {
    scoretk_lilypond::write_lilypond(score)
        .map(String::into_bytes)
        .map_err(|error| Error::LilyPond(error.to_string()))
}

#[cfg(not(feature = "lilypond"))]
pub(crate) fn write_lilypond(_score: &Score) -> Result<Vec<u8>> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Ly,
        sens: Sens::Ecriture,
    })
}

#[cfg(feature = "mscx")]
pub(crate) fn write_mscx(score: &Score) -> Result<Vec<u8>> {
    scoretk_mscx::write_mscx(score)
        .map(String::into_bytes)
        .map_err(|error| Error::Mscx(error.to_string()))
}

#[cfg(not(feature = "mscx"))]
pub(crate) fn write_mscx(_score: &Score) -> Result<Vec<u8>> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Mscx,
        sens: Sens::Ecriture,
    })
}

#[cfg(feature = "mscx")]
pub(crate) fn write_mscz(score: &Score) -> Result<Vec<u8>> {
    scoretk_mscx::write_mscz(score).map_err(|error| Error::Mscx(error.to_string()))
}

#[cfg(not(feature = "mscx"))]
pub(crate) fn write_mscz(_score: &Score) -> Result<Vec<u8>> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Mscz,
        sens: Sens::Ecriture,
    })
}
