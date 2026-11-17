// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
//
// Portage de `engraving/infrastructure/mscreader.cpp` de MuseScore Studio,
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

//! Lecture du conteneur `.mscz` et du fichier `.mscx` principal.
//!
//! Un `.mscz` est une archive ZIP. Le fichier racine est désigné par
//! `META-INF/container.xml` (élément `rootfile`, attribut `full-path`). En
//! l'absence de cette information, on retombe sur le premier `.mscx` situé à
//! la racine de l'archive, comme le fait `MscReader::readScoreFile` dans
//! MuseScore.
//!
//! Un `.mscx` nu est également accepté : il est alors traité comme une archive
//! d'un seul fichier.

use std::fmt;
use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use zip::ZipArchive;

use crate::{Error, Result};

/// Nom conventionnel donné au fichier principal lorsqu'on lit un `.mscx` nu.
pub const DEFAULT_MSCX_NAME: &str = "score.mscx";
const CONTAINER_XML: &str = "META-INF/container.xml";

/// Version du format de fichier MuseScore.
///
/// Encodée comme dans le lecteur C++ (`mscloader.cpp`) : partie majeure
/// multipliée par 100, plus la partie mineure. Par exemple `"3.02"` donne
/// `302` et `"4.60"` donne `460`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MscVersion(u32);

impl MscVersion {
    /// Construit une version à partir de sa représentation numérique (`302`).
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Représentation numérique (`302`, `460`).
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Partie majeure (`3`, `4`).
    pub const fn major(self) -> u32 {
        self.0 / 100
    }

    /// Partie mineure (`2`, `60`).
    pub const fn minor(self) -> u32 {
        self.0 % 100
    }

    /// Analyse un attribut `version` tel qu'écrit dans le fichier
    /// (`"3.02"`, `"4.60"`).
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        let invalid = || Error::InvalidVersion(text.to_owned());

        let (major, minor) = text.split_once('.').ok_or_else(invalid)?;
        if major.is_empty()
            || minor.is_empty()
            || !major.bytes().all(|b| b.is_ascii_digit())
            || !minor.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid());
        }

        let major: u32 = major.parse().map_err(|_| invalid())?;
        let minor: u32 = minor.parse().map_err(|_| invalid())?;
        Ok(Self(major * 100 + minor))
    }
}

impl fmt::Display for MscVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.major(), self.minor())
    }
}

/// Conteneur `.mscz` déjà décodé, ou `.mscx` nu.
///
/// Toutes les données restent en mémoire ; aucune ressource externe n'est
/// ouverte.
#[derive(Debug, Clone)]
pub struct MsczContainer {
    main_mscx_name: String,
    main_mscx: Vec<u8>,
}

impl MsczContainer {
    /// Lit un `.mscz` (archive ZIP) ou un `.mscx` nu à partir de ses octets.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if looks_like_xml(bytes) {
            return Ok(Self {
                main_mscx_name: DEFAULT_MSCX_NAME.to_owned(),
                main_mscx: bytes.to_vec(),
            });
        }

        let mut archive = ZipArchive::new(Cursor::new(bytes))?;

        // 1. Voie normale : le fichier racine annoncé par container.xml.
        if let Some(container) = read_zip_entry(&mut archive, CONTAINER_XML)? {
            if let Some(root) = parse_container_root(&container)? {
                match read_zip_entry(&mut archive, &root)? {
                    Some(data) => {
                        return Ok(Self {
                            main_mscx_name: root,
                            main_mscx: data,
                        });
                    }
                    None => {
                        // container.xml pointe vers un fichier absent : on
                        // essaie le repli avant d'abandonner.
                        if let Some(data) = read_first_root_mscx(&mut archive)? {
                            return Ok(data);
                        }
                        return Err(Error::RootFileNotFound(root));
                    }
                }
            }
        }

        // 2. Repli : premier `.mscx` à la racine de l'archive.
        if let Some(data) = read_first_root_mscx(&mut archive)? {
            return Ok(data);
        }

        Err(Error::NoMscxFound)
    }

    /// Nom du fichier `.mscx` principal dans le conteneur.
    ///
    /// Pour un `.mscx` nu, vaut [`DEFAULT_MSCX_NAME`].
    pub fn main_mscx_name(&self) -> &str {
        &self.main_mscx_name
    }

    /// Contenu XML du fichier `.mscx` principal.
    pub fn main_mscx(&self) -> &[u8] {
        &self.main_mscx
    }

    /// Version déclarée par le fichier `.mscx` principal.
    pub fn version(&self) -> Result<MscVersion> {
        detect_version(self.main_mscx())
    }
}

/// Détecte la version MuseScore d'un `.mscx` en lisant l'attribut `version`
/// de l'élément racine `<museScore>`.
pub fn detect_version(mscx: &[u8]) -> Result<MscVersion> {
    let mut reader = Reader::from_reader(mscx);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "museScore" => {
                let value = find_version_attribute(&e)?.ok_or(Error::MissingVersion)?;
                return MscVersion::parse(&value);
            }
            Event::Eof => return Err(Error::MissingVersion),
            _ => {}
        }
    }
}

fn find_version_attribute(element: &BytesStart<'_>) -> Result<Option<String>> {
    for attribute in element.attributes() {
        let attribute = attribute?;
        if attribute.key.as_ref() == "version" {
            return Ok(Some(
                attribute
                    .normalized_value(XmlVersion::Implicit1_0)?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

/// Vrai si les octets ressemblent à du XML (`.mscx` nu) plutôt qu'à un ZIP.
fn looks_like_xml(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    matches!(bytes.iter().find(|b| !b.is_ascii_whitespace()), Some(b'<'))
}

fn read_zip_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut entry) => {
            let mut data = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut data)
                .map_err(zip::result::ZipError::Io)?;
            Ok(Some(data))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn read_first_root_mscx(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<Option<MsczContainer>> {
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    let candidate = names
        .into_iter()
        .find(|name| !name.contains('/') && name.to_ascii_lowercase().ends_with(".mscx"));

    match candidate {
        Some(name) => {
            let data = read_zip_entry(archive, &name)?
                .ok_or_else(|| Error::RootFileNotFound(name.clone()))?;
            Ok(Some(MsczContainer {
                main_mscx_name: name,
                main_mscx: data,
            }))
        }
        None => Ok(None),
    }
}

/// Extrait le `full-path` du premier `rootfile` de container.xml qui désigne
/// un `.mscx`.
///
/// Comme `MscReader::mainFileName` dans MuseScore, on ne prend pas
/// aveuglément le premier `rootfile` : un `.mscz` écrit par MuseScore 4 liste
/// d'abord `score_style.mss`, puis le `.mscx`. Les entrées qui ne sont pas des
/// `.mscx` (styles, vignette, réglages) sont donc ignorées. S'il n'y a aucun
/// `rootfile` en `.mscx`, on renvoie `None` pour laisser l'appelant retomber
/// sur le premier `.mscx` à la racine de l'archive.
fn parse_container_root(container_xml: &[u8]) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(container_xml);
    reader.config_mut().trim_text(true);

    let mut saw_rootfile = false;
    let mut saw_full_path = false;
    let mut mscx_root = None;
    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "rootfile" => {
                saw_rootfile = true;
                for attribute in e.attributes() {
                    let attribute = attribute?;
                    if attribute.key.as_ref() != "full-path" {
                        continue;
                    }
                    let path = attribute
                        .normalized_value(XmlVersion::Implicit1_0)?
                        .into_owned();
                    let path = path.trim().to_owned();
                    if path.is_empty() {
                        return Err(Error::NoRootFile);
                    }
                    saw_full_path = true;
                    if mscx_root.is_none() && path.to_ascii_lowercase().ends_with(".mscx") {
                        mscx_root = Some(path);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if saw_rootfile && !saw_full_path {
        return Err(Error::NoRootFile);
    }
    Ok(mscx_root)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    const MSCX_460: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.60">
  <Score>
    <Part id="1"/>
  </Score>
</museScore>
"#;

    fn container_xml(root: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<container>
  <rootfiles>
    <rootfile full-path="{root}"/>
  </rootfiles>
</container>
"#
        )
        .into_bytes()
    }

    /// container.xml tel qu'écrit par MuseScore 4.0.1 : le `.mscx` n'est pas
    /// la première entrée, loin s'en faut.
    const CONTAINER_XML_401: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<container>
  <rootfiles>
    <rootfile full-path="score_style.mss"/>
    <rootfile full-path="X.mscx"/>
    <rootfile full-path="Thumbnails/thumbnail.png"/>
    <rootfile full-path="audiosettings.json"/>
    <rootfile full-path="viewsettings.json"/>
  </rootfiles>
</container>
"#;

    fn build_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_mscx_through_container_xml() {
        let zip = build_zip(&[
            ("META-INF/container.xml", &container_xml("score.mscx")),
            ("score.mscx", MSCX_460),
            ("Thumbnails/thumbnail.png", b"png"),
        ]);

        let container = MsczContainer::from_bytes(&zip).unwrap();
        assert_eq!(container.main_mscx_name(), "score.mscx");
        assert_eq!(container.main_mscx(), MSCX_460);
        assert_eq!(container.version().unwrap().as_u32(), 460);
        assert_eq!(container.version().unwrap().to_string(), "4.60");
    }

    #[test]
    fn picks_first_mscx_rootfile_from_musescore_401_container() {
        let zip = build_zip(&[
            ("META-INF/container.xml", CONTAINER_XML_401),
            ("score_style.mss", br#"<museScore version="4.60"/>"#),
            ("X.mscx", MSCX_460),
            ("Thumbnails/thumbnail.png", b"png"),
            ("audiosettings.json", b"{}"),
            ("viewsettings.json", b"{}"),
        ]);

        let container = MsczContainer::from_bytes(&zip).unwrap();
        assert_eq!(container.main_mscx_name(), "X.mscx");
        assert_eq!(container.main_mscx(), MSCX_460);
        assert_eq!(container.version().unwrap().as_u32(), 460);
    }

    #[test]
    fn falls_back_to_root_mscx_when_no_rootfile_is_mscx() {
        let zip = build_zip(&[
            ("META-INF/container.xml", &container_xml("score_style.mss")),
            ("score_style.mss", br#"<museScore version="4.60"/>"#),
            ("Thumbnails/thumbnail.png", b"png"),
            ("Excerpts/part.mscx", br#"<museScore version="4.60"/>"#),
            ("my_score.mscx", MSCX_460),
        ]);

        let container = MsczContainer::from_bytes(&zip).unwrap();
        assert_eq!(container.main_mscx_name(), "my_score.mscx");
        assert_eq!(container.version().unwrap().as_u32(), 460);
    }

    #[test]
    fn extracts_nested_root_file_named_by_container() {
        let zip = build_zip(&[
            (
                "META-INF/container.xml",
                &container_xml("Scores/fugue.mscx"),
            ),
            ("Scores/fugue.mscx", MSCX_460),
        ]);

        let container = MsczContainer::from_bytes(&zip).unwrap();
        assert_eq!(container.main_mscx_name(), "Scores/fugue.mscx");
        assert_eq!(container.version().unwrap().as_u32(), 460);
    }

    #[test]
    fn accepts_raw_mscx() {
        let container = MsczContainer::from_bytes(MSCX_460).unwrap();
        assert_eq!(container.main_mscx_name(), DEFAULT_MSCX_NAME);
        assert_eq!(container.main_mscx(), MSCX_460);
        assert_eq!(detect_version(container.main_mscx()).unwrap().as_u32(), 460);
    }

    #[test]
    fn detects_version_302() {
        let mscx = br#"<museScore version="3.02"><Score/></museScore>"#;
        assert_eq!(detect_version(mscx).unwrap().as_u32(), 302);
        assert_eq!(detect_version(mscx).unwrap().major(), 3);
        assert_eq!(detect_version(mscx).unwrap().minor(), 2);
    }

    #[test]
    fn falls_back_to_first_root_mscx() {
        let zip = build_zip(&[
            ("Thumbnails/thumbnail.png", b"png"),
            ("Excerpts/part.mscx", br#"<museScore version="4.60"/>"#),
            ("my_score.mscx", MSCX_460),
            ("readme.txt", b"hello"),
        ]);

        let container = MsczContainer::from_bytes(&zip).unwrap();
        assert_eq!(container.main_mscx_name(), "my_score.mscx");
        assert_eq!(container.version().unwrap().as_u32(), 460);
    }

    #[test]
    fn errors_when_zip_has_no_mscx() {
        let zip = build_zip(&[
            ("META-INF/container.xml", &container_xml("score.mscx")),
            ("readme.txt", b"nothing here"),
        ]);

        let err = MsczContainer::from_bytes(&zip).unwrap_err();
        assert!(matches!(err, Error::RootFileNotFound(ref n) if n == "score.mscx"));
    }

    #[test]
    fn errors_when_zip_has_neither_container_nor_mscx() {
        let zip = build_zip(&[("readme.txt", b"nothing here")]);
        let err = MsczContainer::from_bytes(&zip).unwrap_err();
        assert!(matches!(err, Error::NoMscxFound));
    }

    #[test]
    fn errors_on_missing_version() {
        let err = detect_version(br#"<museScore><Score/></museScore>"#).unwrap_err();
        assert!(matches!(err, Error::MissingVersion));
    }

    #[test]
    fn errors_on_invalid_version() {
        let err = detect_version(br#"<museScore version="quatre"/>"#).unwrap_err();
        assert!(matches!(err, Error::InvalidVersion(ref v) if v == "quatre"));
    }
}
