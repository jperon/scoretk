// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/writer.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026 :
// `Writer::writeScore`.
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

//! Point d'entrée de l'écriture d'un `.mscx`.
//!
//! `Writer::writeScore` ouvre `<museScore>`, écrit `<Score>` (division, style,
//! métadonnées, parties, portées) et referme le document.

use scoretk_core::model::Score;

use crate::write::context::WriteContext;
use crate::write::staffwrite::{write_meta_tags, write_parts, write_staff_music, write_style};
use crate::xmlwriter::XmlWriter;

/// Version du format écrite dans `<museScore version="…">`.
///
/// MuseScore 4.70 lit tous les éléments produits ici (les mêmes que ceux du
/// lecteur `read4xx`).
pub const MSC_VERSION: &str = "4.70";

/// Version annoncée dans `<programVersion>`.
pub const PROGRAM_VERSION: &str = "4.7.4";

/// Écrit la partition complète en `.mscx`.
pub fn write_score(score: &Score) -> String {
    let context = WriteContext::new(score);
    let mut xml = XmlWriter::new();
    xml.start_document();
    xml.start_element("museScore", &[("version", MSC_VERSION)]);
    xml.tag("programVersion", PROGRAM_VERSION);
    xml.tag("programRevision", "");
    xml.start_element("Score", &[]);
    xml.tag("Division", &score.division.to_string());
    write_style(&mut xml, &score.engraving.style);
    write_meta_tags(&mut xml, &score.metadata);
    write_parts(&mut xml, &context);
    for staff_index in 0..score.staves.len() {
        write_staff_music(&mut xml, &context, staff_index);
    }
    xml.end_element();
    xml.end_element();
    xml.finish()
}
