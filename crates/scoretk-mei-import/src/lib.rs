// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiimporter.cpp` et
// `meiconverter.cpp` de MuseScore Studio 4.7.4, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Lecture de MEI 5.1 (`mei-basic`) vers le modèle pivot.
//!
//! `MeiImporter::read` cherche le premier `./music//score`
//! (`meiimporter.cpp:1011`) puis lit `scoreDef`, les `section`s et leurs
//! mesures. Les éléments inconnus sont ignorés et comptés dans
//! `Score::report` : comme MuseScore, l'import est un analyseur « best
//! effort » et non un validateur RelaxNG.

mod barlines;
mod capabilities;
mod controls;
mod header;
mod reader;

pub use capabilities::CAPABILITIES;
pub use scoretk_mei_common::Error;

use scoretk_core::model::Score;
use scoretk_xml_common::Element;

/// Lit un document MEI en mémoire et renvoie le modèle pivot.
pub fn read_mei(bytes: &[u8]) -> Result<Score, Error> {
    let root = scoretk_xml_common::parse(bytes)?;
    if root.name != "mei" {
        return Err(Error::Unsupported(format!(
            "racine « {} » (attendu mei)",
            root.name
        )));
    }
    reader::Reader::read(&root)
}

/// Premier descendant portant ce nom, dans l'ordre du document.
pub(crate) fn first_descendant<'a>(node: &'a Element, name: &str) -> Option<&'a Element> {
    if node.name == name {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| first_descendant(child, name))
}

/// Texte direct d'un élément, ou texte du premier enfant texte/rend.
pub(crate) fn lines_of(node: &Element) -> String {
    let mut lines = Vec::new();
    if !node.text_trimmed().is_empty() {
        lines.push(node.text_trimmed().to_owned());
    }
    for child in &node.children {
        match child.name.as_str() {
            "lb" => lines.push(String::new()),
            "rend" | "hi" => {
                let text = lines_of(child);
                if !text.is_empty() {
                    lines.push(text);
                }
            }
            _ => {}
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_non_mei_root() {
        assert!(read_mei(b"<score-partwise/>").is_err());
    }

    #[test]
    fn reads_a_minimal_score() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4">
              <clef shape="G" line="2"/></staffDef></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <note dur="4" pname="c" oct="4"/><rest dur="4"/>
            </layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("minimal MEI");
        assert_eq!(score.staves.len(), 1);
        assert_eq!(score.measures.len(), 1);
        assert_eq!(score.notes.len(), 1);
        assert_eq!(score.notes[0].pitch, 60);
    }
}
