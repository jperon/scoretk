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
//! - feature `musicxml-import` : crate `scoretk-musicxml` (t34, t43), fonctions
//!   `read_musicxml(&[u8]) -> Result<Score>` et
//!   `read_mxl(&[u8]) -> Result<Score>` ;
//! - feature `midi` : crate `scoretk-midi` (t35, t42), fonctions
//!   `write_midi(&Score, &Options) -> Result<Vec<u8>>`,
//!   `write_playback(&Score, &Options) -> Result<(Vec<u8>, Timemap)>` et
//!   `read_midi(&[u8]) -> Result<Score>` ;
//! - feature `lilypond` : crate `scoretk-lilypond` (t36), fonction
//!   `write_lilypond(&Score) -> Result<String>` ;
//! - feature `lilypond-import` : lecture LilyPond (t40), fonction
//!   `read_lilypond(&str) -> Result<Score>` ;
//! - feature `mscx` : crate `scoretk-mscx` (t38), fonctions
//!   `write_mscx(&Score) -> Result<String>` et
//!   `write_mscz(&Score) -> Result<Vec<u8>>` ;
//! - feature `abc` : crate `scoretk-abc` (t41), fonctions
//!   `read_abc(&str) -> Result<Score>` et
//!   `write_abc(&Score) -> Result<String>`.

use scoretk_core::model::Score;

use crate::error::Error;
#[cfg(not(all(
    feature = "musicxml-import",
    feature = "midi",
    feature = "lilypond",
    feature = "lilypond-import",
    feature = "mscx",
    feature = "abc"
)))]
use crate::format::{Format, Sens};
use crate::options::Options;
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

#[cfg(feature = "musicxml-import")]
pub(crate) fn read_mxl(bytes: &[u8]) -> Result<Score> {
    scoretk_musicxml::read_mxl(bytes).map_err(|error| Error::MusicXmlImport(error.to_string()))
}

#[cfg(not(feature = "musicxml-import"))]
pub(crate) fn read_mxl(_bytes: &[u8]) -> Result<Score> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Mxl,
        sens: Sens::Lecture,
    })
}

#[cfg(feature = "midi")]
pub(crate) fn write_midi(score: &Score, options: &Options) -> Result<Vec<u8>> {
    scoretk_midi::write_midi_with(
        score,
        scoretk_midi::MidiOptions {
            expand_repeats: options.midi.expand_repeats,
        },
    )
    .map_err(|error| Error::Midi(error.to_string()))
}

#[cfg(not(feature = "midi"))]
pub(crate) fn write_midi(_score: &Score, _options: &Options) -> Result<Vec<u8>> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Midi,
        sens: Sens::Ecriture,
    })
}

/// Écrit la sortie MIDI et renvoie aussi la carte des temps de la performance.
///
/// Sous-produit du même rendu : les octets MIDI sont identiques à ceux de
/// [`write_midi`] pour les mêmes options.
#[cfg(feature = "midi")]
pub(crate) fn write_playback(
    score: &Score,
    options: &Options,
) -> Result<(Vec<u8>, scoretk_midi::Timemap)> {
    scoretk_midi::write_midi_with_timemap_and(
        score,
        scoretk_midi::MidiOptions {
            expand_repeats: options.midi.expand_repeats,
        },
    )
    .map_err(|error| Error::Midi(error.to_string()))
}

#[cfg(feature = "midi")]
pub(crate) fn read_midi(bytes: &[u8]) -> Result<Score> {
    scoretk_midi::read_midi(bytes).map_err(|error| Error::MidiImport(error.to_string()))
}

#[cfg(not(feature = "midi"))]
pub(crate) fn read_midi(_bytes: &[u8]) -> Result<Score> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Midi,
        sens: Sens::Lecture,
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

#[cfg(feature = "lilypond-import")]
pub(crate) fn read_lilypond(bytes: &[u8]) -> Result<Score> {
    let source = std::str::from_utf8(bytes).map_err(|error| Error::LilyPond(error.to_string()))?;
    scoretk_lilypond::read_lilypond(source).map_err(|error| Error::LilyPond(error.to_string()))
}

#[cfg(not(feature = "lilypond-import"))]
pub(crate) fn read_lilypond(_bytes: &[u8]) -> Result<Score> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Ly,
        sens: Sens::Lecture,
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

#[cfg(feature = "abc")]
pub(crate) fn read_abc(bytes: &[u8]) -> Result<Score> {
    let text = std::str::from_utf8(bytes).map_err(|error| Error::Abc(error.to_string()))?;
    scoretk_abc::read_abc(text).map_err(|error| Error::Abc(error.to_string()))
}

#[cfg(not(feature = "abc"))]
pub(crate) fn read_abc(_bytes: &[u8]) -> Result<Score> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Abc,
        sens: Sens::Lecture,
    })
}

#[cfg(feature = "abc")]
pub(crate) fn write_abc(score: &Score) -> Result<Vec<u8>> {
    scoretk_abc::write_abc(score)
        .map(String::into_bytes)
        .map_err(|error| Error::Abc(error.to_string()))
}

#[cfg(not(feature = "abc"))]
pub(crate) fn write_abc(_score: &Score) -> Result<Vec<u8>> {
    Err(Error::FormatNonPrisEnCharge {
        format: Format::Abc,
        sens: Sens::Ecriture,
    })
}
