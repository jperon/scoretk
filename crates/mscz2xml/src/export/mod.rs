// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// Portage de `importexport/musicxml/internal/export/` de MuseScore Studio,
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

//! Export MusicXML.
//!
//! Point d'entrée du portage de
//! `importexport/musicxml/internal/export/exportmusicxml.cpp`.
//!
//! - [`export_musicxml`] produit un document MusicXML `partwise` 4.0 en
//!   texte, avec l'en-tête et le DOCTYPE de MuseScore 4.7 ;
//! - [`export_mxl`] produit la variante compressée (`.mxl`) : une archive ZIP
//!   contenant `META-INF/container.xml` et `score.xml`.
//!
//! Les deux fonctions travaillent entièrement en mémoire et n'ouvrent aucun
//! fichier : le module reste utilisable pour `wasm32-unknown-unknown`.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::model::Score;
use crate::Result;

mod beams;
mod exportmusicxml;
mod harmony;
mod layout;
mod notations;
mod nuances;
mod ottava;
mod pedals;
mod pitch;
mod repeats;
mod stems;
mod text;
mod textlines;
mod writer;

#[cfg(test)]
mod tests;

pub use exportmusicxml::{
    calc_divisions, export_musicxml, export_musicxml_with, ExportOptions, DEFAULT_ENCODING_DATE,
    DEFAULT_SOFTWARE,
};

/// Nom du fichier MusicXML placé dans l'archive `.mxl`.
///
/// MuseScore n'a pas de nom de fichier à portée de main lors de l'export en
/// mémoire : il utilise `score.xml`, valeur reprise ici pour rester
/// déterministe.
pub const MXL_SCORE_FILE_NAME: &str = "score.xml";

/// Nom de l'entrée de conteneur imposée par la spécification `.mxl`.
pub const MXL_CONTAINER_FILE_NAME: &str = "META-INF/container.xml";

/// Exporte une partition en MusicXML compressé (`.mxl`).
///
/// Le fichier `META-INF/container.xml` désigne [`MXL_SCORE_FILE_NAME`], comme
/// `writeMxlArchive` dans `exportmusicxml.cpp`. L'archive est déterministe :
/// compression `deflate` et horodatage ZIP figé à 1980-01-01.
pub fn export_mxl(score: &Score) -> Result<Vec<u8>> {
    export_mxl_with(score, &ExportOptions::default())
}

/// Variante de [`export_mxl`] avec options.
pub fn export_mxl_with(score: &Score, options: &ExportOptions) -> Result<Vec<u8>> {
    let musicxml = export_musicxml_with(score, options)?;
    let container = container_xml();

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let file_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());

    writer.start_file(MXL_CONTAINER_FILE_NAME, file_options)?;
    writer
        .write_all(container.as_bytes())
        .map_err(zip::result::ZipError::Io)?;
    writer.start_file(MXL_SCORE_FILE_NAME, file_options)?;
    writer
        .write_all(musicxml.as_bytes())
        .map_err(zip::result::ZipError::Io)?;

    Ok(writer.finish()?.into_inner())
}

/// Construit `META-INF/container.xml`, avec l'indentation de MuseScore.
fn container_xml() -> String {
    let mut xml = writer::XmlWriter::new();
    xml.start_document();
    xml.start_element("container", &[]);
    xml.start_element("rootfiles", &[]);
    xml.start_element("rootfile", &[("full-path", MXL_SCORE_FILE_NAME)]);
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.finish()
}
