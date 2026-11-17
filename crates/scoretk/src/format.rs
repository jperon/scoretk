// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage partiel de `importexport/musicxml/internal/musicxmlreader.cpp` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026 : reconnaissance du format par suffixe, comme
// `MusicXmlReader::read` (`xml`/`musicxml` contre `mxl`).
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
/// `Ly` est reconnu (extension et contenu) et s'écrit avec la feature
/// `lilypond` ; il n'a pas encore de lecteur. `Abc` se lit et s'écrit avec la
/// feature `abc`. [`read`](crate::read) et [`write`](crate::write) renvoient
/// [`Error::FormatNonPrisEnCharge`] quand la feature manque.
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
    /// LilyPond (`.ly`).
    Ly,
    /// ABC (`.abc`).
    Abc,
    /// MIDI standard (`.mid`, `.midi`).
    Midi,
}

impl Format {
    /// Tous les formats, dans l'ordre d'affichage de `--list-formats`.
    pub const ALL: [Self; 7] = [
        Self::Mscz,
        Self::Mscx,
        Self::MusicXml,
        Self::Mxl,
        Self::Ly,
        Self::Abc,
        Self::Midi,
    ];

    /// Nom canonique (celui de `--from`/`--to`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mscz => "mscz",
            Self::Mscx => "mscx",
            Self::MusicXml => "musicxml",
            Self::Mxl => "mxl",
            Self::Ly => "ly",
            Self::Abc => "abc",
            Self::Midi => "midi",
        }
    }

    /// Extensions associées, sans le point, dans l'ordre de préférence.
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Mscz => &["mscz"],
            Self::Mscx => &["mscx"],
            Self::MusicXml => &["musicxml", "xml"],
            Self::Mxl => &["mxl"],
            Self::Ly => &["ly"],
            Self::Abc => &["abc"],
            Self::Midi => &["mid", "midi"],
        }
    }

    /// Vrai si [`read`](crate::read) sait lire ce format aujourd'hui.
    ///
    /// MusicXML dépend de la feature `musicxml-import`, ABC de la feature
    /// `abc`, MIDI n'a pas encore de lecteur.
    pub const fn supports_read(self) -> bool {
        match self {
            Self::Mscz | Self::Mscx => true,
            Self::MusicXml => cfg!(feature = "musicxml-import"),
            Self::Abc => cfg!(feature = "abc"),
            Self::Mxl | Self::Ly | Self::Midi => false,
        }
    }

    /// Vrai si [`write`](crate::write) sait écrire ce format aujourd'hui.
    ///
    /// MuseScore (`.mscx`, `.mscz`) dépend de la feature `mscx`, MIDI de la
    /// feature `midi`, LilyPond de la feature `lilypond`, ABC de la feature
    /// `abc`.
    pub const fn supports_write(self) -> bool {
        match self {
            Self::MusicXml | Self::Mxl => true,
            Self::Mscz | Self::Mscx => cfg!(feature = "mscx"),
            Self::Midi => cfg!(feature = "midi"),
            Self::Ly => cfg!(feature = "lilypond"),
            Self::Abc => cfg!(feature = "abc"),
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
            "ly" | "lilypond" => Some(Self::Ly),
            "abc" => Some(Self::Abc),
            "mid" | "midi" => Some(Self::Midi),
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
    ///    sinon `.mxl` s'il contient un `.xml`/`.musicxml` ;
    /// 2. `MThd` : MIDI ;
    /// 3. `<museScore` : `.mscx` ;
    /// 4. `<score-partwise`/`<score-timewise` ou `<?xml` : MusicXML ;
    /// 5. `\version` : LilyPond ;
    /// 6. `%abc` ou un en-tête `X:` : ABC.
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
        if contains(text, b"<score-partwise") || contains(text, b"<score-timewise") {
            return Some(Self::MusicXml);
        }
        if contains(text, b"<?xml") {
            // XML non identifié plus précisément : MusicXML est le seul format
            // XML que scoretk manipule, et l'appelant peut toujours forcer
            // `--from`.
            return Some(Self::MusicXml);
        }
        if text.starts_with(b"\\version") {
            return Some(Self::Ly);
        }
        if text.starts_with(b"%abc") || text.starts_with(b"X:") || contains(text, b"\nX:") {
            return Some(Self::Abc);
        }
        None
    }
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
fn detect_zip(bytes: &[u8]) -> Option<Format> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut has_mscx = false;
    let mut has_xml = false;

    for index in 0..archive.len() {
        let name = archive.by_index(index).ok()?.name().to_ascii_lowercase();
        has_mscx |= name.ends_with(".mscx");
        has_xml |= name.ends_with(".xml") || name.ends_with(".musicxml");
    }

    if has_mscx {
        Some(Format::Mscz)
    } else if has_xml {
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
