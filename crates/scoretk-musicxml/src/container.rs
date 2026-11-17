// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxml.cpp` et du
// lecteur de conteneur `engraving/infrastructure/mscreader.cpp` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026.
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

//! Extraction du document MusicXML d'un conteneur `.mxl`.
//!
//! Un `.mxl` est une archive ZIP dont `META-INF/container.xml` désigne le
//! fichier racine (premier `<rootfile full-path="…">`, comme
//! `MusicXmlReader::readMusicXml`). Un `.musicxml` ou `.xml` nu est accepté
//! tel quel.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

use crate::error::{Error, Result};

const CONTAINER_XML: &str = "META-INF/container.xml";

/// Extrait le document MusicXML d'un `.mxl` ou renvoie les octets tels quels.
pub(crate) fn extract(bytes: &[u8]) -> Result<Vec<u8>> {
    if looks_like_xml(bytes) {
        return Ok(bytes.to_vec());
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes))?;

    if let Some(container) = read_entry(&mut archive, CONTAINER_XML)? {
        let root = parse_rootfile(&container)?;
        match read_entry(&mut archive, &root)? {
            Some(data) => return Ok(data),
            None => return Err(Error::RootFileNotFound(root)),
        }
    }

    // Repli : premier fichier `.musicxml`/`.xml` hors `META-INF`, dans l'ordre
    // de l'archive.
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    for name in names {
        if name.starts_with("META-INF/") {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".musicxml") || lower.ends_with(".xml") {
            if let Some(data) = read_entry(&mut archive, &name)? {
                return Ok(data);
            }
        }
    }
    Err(Error::NoRootFile)
}

/// Vrai si les octets commencent par un `<` (après espaces et BOM).
fn looks_like_xml(bytes: &[u8]) -> bool {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    matches!(bytes.get(start), Some(b'<'))
}

fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Option<Vec<u8>>> {
    let mut file = match archive.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(Error::Zip(error)),
    };
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|error| Error::Xml(quick_xml::Error::Io(std::sync::Arc::new(error))))?;
    Ok(Some(data))
}

/// Premier `full-path` de `container.xml`.
fn parse_rootfile(container: &[u8]) -> Result<String> {
    let mut reader = Reader::from_reader(container);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                let name = event.name();
                if name.as_ref().rsplit(':').next() == Some("rootfile") {
                    for attribute in event.attributes() {
                        let attribute = attribute?;
                        let key = attribute.key;
                        if key.as_ref().rsplit(':').next() == Some("full-path") {
                            let value = attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map(|value| value.into_owned())
                                .unwrap_or_default();
                            if !value.is_empty() {
                                return Ok(value);
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(Error::Xml(error)),
        }
    }
    Err(Error::NoRootFile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_nu_passe_tel_quel() {
        let bytes = b"<score-partwise/>";
        assert_eq!(extract(bytes).unwrap(), bytes);
    }
}
