// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxml.cpp` de
// MuseScore Studio (`importMusicXml`), Copyright (C) MuseScore Limited and
// others, modifié (réécrit en Rust) en 2026 : acceptation d'un document nu ou
// d'une archive `.mxl`. L'extraction du conteneur ZIP est portée dans `mxl.rs`
// (`extractRootfile`).
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

//! Choix entre document MusicXML nu et conteneur `.mxl`.
//!
//! Un `.musicxml` ou `.xml` nu est accepté tel quel ; tout le reste est
//! traité comme une archive `.mxl` par [`crate::mxl::extract_rootfile`].

use crate::error::Result;

/// Extrait le document MusicXML d'un `.mxl` ou renvoie les octets tels quels.
pub(crate) fn extract(bytes: &[u8]) -> Result<Vec<u8>> {
    if looks_like_xml(bytes) {
        return Ok(bytes.to_vec());
    }
    super::mxl::extract_rootfile(bytes)
}

/// Vrai si les octets commencent par un `<` (après espaces et BOM).
fn looks_like_xml(bytes: &[u8]) -> bool {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    matches!(bytes.get(start), Some(b'<'))
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
