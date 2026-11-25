// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage partiel de `importexport/musicxml/internal/musicxmlreader.cpp` et de
// `importexport/musicxml/internal/import/importmusicxml.cpp`
// (`importCompressedMusicXml`) de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026 : reconnaissance du
// format par suffixe, comme `MusicXmlReader::read` (`xml`/`musicxml` contre
// `mxl`), et détection d'un `.mxl` par `META-INF/container.xml`.
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

//! Formats de partition reconnus par scoretk, et détection.

use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::str::FromStr;

use crate::error::Error;

/// Sens d'une conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sens {
    /// Fichier → modèle.
    Lecture,
    /// Modèle → fichier.
    Ecriture,
}

impl Sens {
    /// Nom français, pour les messages et `--list-formats`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lecture => "lecture",
            Self::Ecriture => "écriture",
        }
    }
}

impl fmt::Display for Sens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Format de partition manipulé par scoretk.
///
/// Chaque format a deux features : `<format>-import` pour [`read`](crate::read)
/// et `<format>-export` pour [`write`](crate::write). Les deux fonctions
/// renvoient [`Error::FormatNonPrisEnCharge`] quand la feature du sens demandé
/// manque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// Conteneur MuseScore compressé (`.mscz`, archive ZIP avec un `.mscx`).
    Mscz,
    /// Partition MuseScore en XML (`.mscx`).
    Mscx,
    /// MusicXML `partwise`/`timewise` non compressé (`.musicxml`, `.xml`).
    MusicXml,
    /// MusicXML compressé (`.mxl`, archive ZIP avec `META-INF/container.xml`).
    Mxl,
    /// MEI 5.1 (`mei-basic`), espace de noms `music-encoding.org/ns/mei`.
    Mei,
    /// LilyPond (`.ly`).
    Ly,
    /// ABC (`.abc`).
    Abc,
    /// MIDI standard (`.mid`, `.midi`).
    Midi,
    /// Gregorian GABC, syntaxe de Gregorio (`.gabc`).
    Gabc,
}

impl Format {
    /// Tous les formats, dans l'ordre d'affichage de `--list-formats`.
    pub const ALL: [Self; 9] = [
        Self::Mscz,
        Self::Mscx,
        Self::MusicXml,
        Self::Mxl,
        Self::Mei,
        Self::Ly,
        Self::Abc,
        Self::Midi,
        Self::Gabc,
    ];

    /// Nom canonique (celui de `--from`/`--to`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mscz => "mscz",
            Self::Mscx => "mscx",
            Self::MusicXml => "musicxml",
            Self::Mxl => "mxl",
            Self::Mei => "mei",
            Self::Ly => "ly",
            Self::Abc => "abc",
            Self::Midi => "midi",
            Self::Gabc => "gabc",
        }
    }

    /// Extensions associées, sans le point, dans l'ordre de préférence.
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Mscz => &["mscz"],
            Self::Mscx => &["mscx"],
            Self::MusicXml => &["musicxml", "xml"],
            Self::Mxl => &["mxl"],
            Self::Mei => &["mei"],
            Self::Ly => &["ly"],
            Self::Abc => &["abc"],
            Self::Midi => &["mid", "midi"],
            Self::Gabc => &["gabc"],
        }
    }

    /// Vrai si [`read`](crate::read) sait lire ce format.
    ///
    /// Chaque format dépend de sa feature `<format>-import` : MuseScore
    /// `musescore-import` (`.mscz`, `.mscx`), MusicXML `musicxml-import`
    /// (`.musicxml`, `.xml`, `.mxl`), LilyPond `lilypond-import`, ABC
    /// `abc-import`, MIDI `midi-import`, GABC `gabc-import`.
    pub const fn supports_read(self) -> bool {
        match self {
            Self::Mscz | Self::Mscx => cfg!(feature = "musescore-import"),
            Self::MusicXml | Self::Mxl => cfg!(feature = "musicxml-import"),
            Self::Mei => cfg!(feature = "mei-import"),
            Self::Ly => cfg!(feature = "lilypond-import"),
            Self::Abc => cfg!(feature = "abc-import"),
            Self::Midi => cfg!(feature = "midi-import"),
            Self::Gabc => cfg!(feature = "gabc-import"),
        }
    }

    /// Vrai si [`write`](crate::write) sait écrire ce format.
    ///
    /// Chaque format dépend de sa feature `<format>-export` : MuseScore
    /// `musescore-export` (`.mscx`, `.mscz`), MusicXML `musicxml-export`,
    /// LilyPond `lilypond-export`, ABC `abc-export`, MIDI `midi-export`.
    pub const fn supports_write(self) -> bool {
        match self {
            Self::MusicXml | Self::Mxl => cfg!(feature = "musicxml-export"),
            Self::Mei => cfg!(feature = "mei-export"),
            Self::Mscz | Self::Mscx => cfg!(feature = "musescore-export"),
            Self::Midi => cfg!(feature = "midi-export"),
            Self::Ly => cfg!(feature = "lilypond-export"),
            Self::Abc => cfg!(feature = "abc-export"),
            Self::Gabc => cfg!(feature = "gabc-export"),
        }
    }

    /// Reconnaît un format à partir d'une extension, sans le point.
    ///
    /// Insensible à la casse ; `xml` désigne MusicXML et `mid` MIDI, comme
    /// dans MuseScore (`MusicXmlReader::read`).
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.trim().to_ascii_lowercase().as_str() {
            "mscz" => Some(Self::Mscz),
            "mscx" => Some(Self::Mscx),
            "musicxml" | "xml" => Some(Self::MusicXml),
            "mxl" => Some(Self::Mxl),
            "mei" => Some(Self::Mei),
            "ly" | "lilypond" => Some(Self::Ly),
            "abc" => Some(Self::Abc),
            "mid" | "midi" => Some(Self::Midi),
            "gabc" => Some(Self::Gabc),
            _ => None,
        }
    }

    /// Reconnaît un format à partir du nom (ou chemin) d'un fichier.
    ///
    /// Aucun accès au disque n'est fait : seule l'extension est examinée.
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        path.as_ref()
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Self::from_extension)
    }

    /// Devine un format d'après les premiers octets du contenu.
    ///
    /// Ordre des tests :
    ///
    /// 1. signature ZIP (`PK\x03\x04`) : `.mscz` s'il contient un `.mscx`,
    ///    sinon `.mxl` s'il contient `META-INF/container.xml` ou un
    ///    `.xml`/`.musicxml` ;
    /// 2. `MThd` : MIDI ;
    /// 3. `<museScore` : `.mscx` ;
    /// 4. l'espace de noms MEI (`music-encoding.org/ns/mei`) : MEI ;
    /// 5. `<score-partwise`/`<score-timewise` ou `<?xml` : MusicXML ;
    /// 6. `\version` : LilyPond ;
    /// 7. `%abc` ou un en-tête `X:` : ABC ;
    /// 8. un en-tête `name:` suivi d'une ligne `%%` : GABC
    ///    (`Gabc.tex:51,58,79-82`).
    ///
    /// Les tests sont mutuellement exclusifs : un document MEI porte
    /// l'espace de noms MEI (jamais de GABC `%%`), et un `.gabc` ne contient
    /// ni balise XML ni en-tête ABC.
    ///
    /// Renvoie `None` si rien ne correspond : la façade préfère une erreur à
    /// une supposition fausse.
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        if is_zip(bytes) {
            return detect_zip(bytes);
        }
        if bytes.starts_with(b"MThd") {
            return Some(Self::Midi);
        }

        let text = strip_bom_and_space(bytes);
        if contains(text, b"<museScore") {
            return Some(Self::Mscx);
        }
        if contains(text, b"music-encoding.org/ns/mei") || contains(text, b"<mei ") {
            return Some(Self::Mei);
        }
        if contains(text, b"<score-partwise") || contains(text, b"<score-timewise") {
            return Some(Self::MusicXml);
        }
        if contains(text, b"<?xml") {
            // XML non identifié plus précisément : MusicXML est le format
            // XML par défaut ; l'appelant peut toujours forcer `--from`.
            return Some(Self::MusicXml);
        }
        if text.starts_with(b"\\version") {
            return Some(Self::Ly);
        }
        if text.starts_with(b"%abc") || text.starts_with(b"X:") || contains(text, b"\nX:") {
            return Some(Self::Abc);
        }
        if is_gabc(text) {
            return Some(Self::Gabc);
        }
        None
    }
}

/// Une ligne `name:` et une ligne `%%` suffisent à reconnaître un `.gabc`
/// (`Gabc.tex:51,58,79-82`). Les commentaires et les espaces précédant le
/// nom sont autorisés, comme dans les exemples de Gregorio.
fn is_gabc(text: &[u8]) -> bool {
    let source = String::from_utf8_lossy(text);
    let mut has_name = false;
    let mut has_separator = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name:") {
            has_name = true;
        }
        if trimmed == "%%" {
            has_separator = true;
        }
        if has_name && has_separator {
            return true;
        }
    }
    false
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Format {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::from_extension(text).ok_or_else(|| Error::FormatInconnu(text.trim().to_owned()))
    }
}

fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

/// Distingue un `.mscz` d'un `.mxl` par le nom des entrées de l'archive.
///
/// Un `.mscz` est reconnu au premier `.mscx` rencontré : il contient lui aussi
/// un `META-INF/container.xml`, d'où l'ordre des tests. Un `.mxl` est reconnu
/// à son `META-INF/container.xml`, ou à défaut à un `.xml`/`.musicxml` hors du
/// conteneur.
fn detect_zip(bytes: &[u8]) -> Option<Format> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut has_mscx = false;
    let mut has_container = false;
    let mut has_xml = false;

    for index in 0..archive.len() {
        let name = archive.by_index(index).ok()?.name().to_ascii_lowercase();
        has_mscx |= name.ends_with(".mscx");
        has_container |= name == "meta-inf/container.xml";
        has_xml |= name.ends_with(".xml") || name.ends_with(".musicxml");
    }

    if has_mscx {
        Some(Format::Mscz)
    } else if has_container || has_xml {
        Some(Format::Mxl)
    } else {
        None
    }
}

fn strip_bom_and_space(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
