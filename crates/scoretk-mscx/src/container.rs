// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/infrastructure/mscwriter.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026 : conteneur `.mscz` (ZIP) et `META-INF/container.xml`.
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

//! Conteneur `.mscz`.
//!
//! MuseScore écrit une archive ZIP dont le fichier racine est désigné par
//! `META-INF/container.xml`. Le nom du `.mscx` est repris du nom de fichier
//! ouvert ; sans nom de fichier, `MscWriter::mainFileName` retombe sur
//! `score.mscx`. Le style de la partition est écrit à part
//! (`score_style.mss`), comme `MscSaver::writeMscz`, et repris ici pour que
//! MuseScore relise le même style que le `.mscx` en mémoire.

use std::io::{Cursor, Write};

use mscz2xml::model::Score;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::Error;
use crate::write::staffwrite::write_style;
use crate::xmlwriter::XmlWriter;

/// Nom du `.mscx` principal dans l'archive, sans nom de fichier d'origine.
pub const MSCX_FILE_NAME: &str = "score.mscx";

/// Nom du fichier de style (`MscWriter::writeStyleFile`).
pub const STYLE_FILE_NAME: &str = "score_style.mss";

/// Nom de l'entrée de conteneur (`META-INF/container.xml`).
pub const CONTAINER_FILE_NAME: &str = "META-INF/container.xml";

/// Construit l'archive `.mscz` à partir du `.mscx` et du style.
pub(crate) fn write_container(score: &Score, mscx: &str) -> Result<Vec<u8>, Error> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());

    write_entry(&mut writer, CONTAINER_FILE_NAME, &container_xml(), options)?;
    write_entry(&mut writer, STYLE_FILE_NAME, &style_file(score), options)?;
    write_entry(&mut writer, MSCX_FILE_NAME, mscx, options)?;
    write_entry(&mut writer, "audiosettings.json", "{}", options)?;
    write_entry(&mut writer, "viewsettings.json", "{}", options)?;

    Ok(writer.finish()?.into_inner())
}

/// Ajoute une entrée à l'archive.
fn write_entry(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    data: &str,
    options: SimpleFileOptions,
) -> Result<(), Error> {
    writer.start_file(name, options)?;
    writer
        .write_all(data.as_bytes())
        .map_err(zip::result::ZipError::Io)?;
    Ok(())
}

/// Construit `META-INF/container.xml`, dans l'ordre de MuseScore 4.7.
fn container_xml() -> String {
    let mut xml = XmlWriter::new();
    xml.start_document();
    xml.start_element("container", &[]);
    xml.start_element("rootfiles", &[]);
    xml.tag_empty("rootfile", &[("full-path", STYLE_FILE_NAME)]);
    xml.tag_empty("rootfile", &[("full-path", MSCX_FILE_NAME)]);
    xml.tag_empty("rootfile", &[("full-path", "audiosettings.json")]);
    xml.tag_empty("rootfile", &[("full-path", "viewsettings.json")]);
    xml.end_element();
    xml.end_element();
    xml.finish()
}

/// Construit `score_style.mss` à partir du style du modèle.
fn style_file(score: &Score) -> String {
    let mut xml = XmlWriter::new();
    xml.start_document();
    xml.start_element("museScore", &[("version", crate::MSC_VERSION)]);
    write_style(&mut xml, &score.style);
    xml.end_element();
    xml.finish()
}
