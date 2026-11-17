// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxml.cpp` de
// MuseScore Studio (`extractRootfile`, `importCompressedMusicXml`), Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Lecture du MusicXML compressé (`.mxl`).
//!
//! Un `.mxl` est une archive ZIP. `META-INF/container.xml` désigne le document
//! MusicXML par le premier `<rootfile full-path="…">`, comme `extractRootfile`
//! de `importmusicxml.cpp`. Si `container.xml` manque, on se replie sur le
//! premier `.musicxml`/`.xml` hors `META-INF`, dans l'ordre de l'archive (repli
//! propre à scoretk, absent du C++).
//!
//! Tout se passe en mémoire : aucune I/O disque, aucun thread, aucune horloge ;
//! le module compile pour `wasm32-unknown-unknown`.

use std::io::{Cursor, Read};

use mscz2xml::model::Score;
use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

use crate::error::{Error, Result};

/// Nom de l'entrée de conteneur imposée par la spécification `.mxl`.
const CONTAINER_XML: &str = "META-INF/container.xml";

/// Lit un fichier MusicXML compressé (`.mxl`) et renvoie le modèle pivot.
///
/// Équivalent de `importCompressedMusicXml` : extrait le fichier racine de
/// l'archive ([`extract_rootfile`]) puis le fait passer par le lecteur
/// MusicXML commun. Un document MusicXML nu n'est pas accepté ici : c'est le
/// rôle de [`read_musicxml`](crate::read_musicxml).
pub fn read_mxl(bytes: &[u8]) -> Result<Score> {
    let document = extract_rootfile(bytes)?;
    crate::parse_document(&document)
}

/// Extrait le document MusicXML d'une archive `.mxl`.
///
/// Portage de `extractRootfile` (`importmusicxml.cpp`) : lit
/// `META-INF/container.xml`, y prend le premier `full-path` d'un `<rootfile>`,
/// puis lit cette entrée de l'archive. Une erreur est renvoyée si l'archive
/// est illisible, si `container.xml` ne désigne aucun fichier, si le fichier
/// désigné est absent, ou si l'archive ne contient aucun `.musicxml`/`.xml`.
pub(crate) fn extract_rootfile(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;

    if let Some(container) = read_entry(&mut archive, CONTAINER_XML)? {
        let root = parse_rootfile(&container)?;
        return match read_entry(&mut archive, &root)? {
            Some(data) => Ok(data),
            None => Err(Error::RootFileNotFound(root)),
        };
    }

    // Repli : premier `.musicxml`/`.xml` hors `META-INF`, dans l'ordre de
    // l'archive. `ZipArchive::file_names` ne permet pas de lire en même temps,
    // d'où la copie des noms avant l'ouverture des entrées.
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
    Err(Error::NoMusicXmlInMxl)
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
///
/// La descente suit le C++ (`container` → `rootfiles` → `rootfile`), mais
/// tolère un préfixe d'espace de noms sur les trois noms d'éléments.
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
