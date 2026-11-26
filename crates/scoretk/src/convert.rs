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

//! Orchestration historique MuseScore → MusicXML en un appel.

use crate::options::{MidiOptions, Options};
use crate::{read_with_report, write_with_report, Format, Result};
use scoretk_musicxml_export::ExportOptions;

/// Format de sortie de [`convert`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// MusicXML `partwise` non compressé (`.musicxml`, `.xml`).
    #[default]
    MusicXml,
    /// MusicXML compressé, archive ZIP `.mxl`.
    Mxl,
}

/// Options de [`convert`].
///
/// Regroupe le choix du format de sortie et les [`ExportOptions`] transmises à
/// l'export MusicXML (logiciel annoncé, date d'encodage, hauteurs en sons
/// réels).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConvertOptions {
    /// Format de sortie.
    pub format: OutputFormat,
    /// Options transmises à l'export MusicXML.
    pub export: ExportOptions,
}

/// Convertit un `.mscz` ou un `.mscx` en MusicXML, en une seule étape.
///
/// Enchaîne la lecture MuseScore ([`read`]) puis l'écriture MusicXML
/// ([`write`]) ; tout se passe en mémoire. La date d'encodage vient de
/// [`ExportOptions::encoding_date`], ce qui rend les sorties reproductibles.
pub fn convert(input: &[u8], options: &ConvertOptions) -> Result<Vec<u8>> {
    convert_with_report(input, options).map(|(bytes, _)| bytes)
}

/// Variante de [`convert`] qui renvoie aussi le rapport de pertes.
pub fn convert_with_report(
    input: &[u8],
    options: &ConvertOptions,
) -> Result<(Vec<u8>, crate::model::ConversionReport)> {
    let (score, _) = read_with_report(input, Format::Mscz)?;
    let format = match options.format {
        OutputFormat::MusicXml => Format::MusicXml,
        OutputFormat::Mxl => Format::Mxl,
    };
    let write_options = Options {
        software: options.export.software.clone(),
        encoding_date: options.export.encoding_date.clone(),
        concert_pitch: options.export.concert_pitch,
        midi: MidiOptions::default(),
        midi_full_support: false,
        gabc_modern_clef: options.export.gabc_modern_clef,
        mei_full_support: false,
        gabc_neume_smufl: options.export.gabc_neume_smufl,
        ly_gregorian: false,
        xml_full_support: options.export.full_support,
        xml_lossless_round_trip: options.export.lossless_round_trip,
    };
    write_with_report(&score, format, &write_options)
}
