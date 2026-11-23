// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `writeMusicXml(const FretDiagram*)`
// (`importexport/musicxml/internal/export/exportmusicxml.cpp:9049-9122`) de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Grille d'accord (`<frame>` dans `<harmony>`).
//!
//! Portage de `writeMusicXml(const FretDiagram*)`. Les cordes sont numérotées
//! de la plus aiguë (1) à la plus grave (`strings`) ; les points et marqueurs
//! sont rangés par indice interne (`<string no>`, 0 = corde la plus grave).

use scoretk_core::model::{FretDiagram, FretDotType, FretMarkerType};

use super::exportmusicxml::Out;

/// Écrit `<frame>` pour une grille d'accord.
pub(crate) fn write_frame(out: &mut Out, diagram: &FretDiagram) {
    out.xml.start_element("frame", &[]);
    out.xml.tag("frame-strings", &diagram.strings.to_string());
    out.xml.tag("frame-frets", &diagram.frets.to_string());
    if diagram.fret_offset > 0 {
        out.xml
            .tag("first-fret", &(diagram.fret_offset + 1).to_string());
    }

    for index in 0..diagram.strings {
        let mxml_string = diagram.strings - index;
        // Barres qui commencent ou finissent à cette corde.
        let mut starts: Vec<u8> = Vec::new();
        let mut ends: Vec<u8> = Vec::new();
        for barre in &diagram.barres {
            if barre.start == index as i8 {
                starts.push(barre.fret);
            } else if barre.end == index as i8 || (barre.end == -1 && mxml_string == 1) {
                ends.push(barre.fret);
            }
        }

        // Un marqueur cercle se traduit par une corde à vide.
        if diagram
            .markers
            .iter()
            .any(|marker| marker.string == index && marker.kind == FretMarkerType::Circle)
        {
            frame_note(out, mxml_string, 0, None);
        }

        for dot in diagram
            .dots
            .iter()
            .filter(|dot| dot.string == index && dot.kind != FretDotType::Cross)
        {
            let mut barre = None;
            if let Some(position) = starts.iter().position(|fret| *fret == dot.fret) {
                starts.remove(position);
                barre = Some("start");
            }
            if let Some(position) = ends.iter().position(|fret| *fret == dot.fret) {
                ends.remove(position);
                barre = Some("stop");
            }
            frame_note(
                out,
                mxml_string,
                u32::from(dot.fret) + u32::from(diagram.fret_offset),
                barre,
            );
        }

        // Barres dont aucune extrémité n'a de point (le C++ n'ajoute pas
        // `fretOffset` ici).
        for fret in starts {
            frame_note(out, mxml_string, u32::from(fret), Some("start"));
        }
        for fret in ends {
            frame_note(out, mxml_string, u32::from(fret), Some("stop"));
        }
    }

    out.xml.end_element();
}

/// Écrit un `<frame-note>` pour une corde.
fn frame_note(out: &mut Out, string: u8, fret: u32, barre: Option<&str>) {
    if fret == 0 && barre.is_none() {
        // `writeMusicXml(const FretDiagram*)` écrit toujours le `<fret>`,
        // même à zéro (marqueur cercle).
    }
    out.xml.start_element("frame-note", &[]);
    out.xml.tag("string", &string.to_string());
    out.xml.tag("fret", &fret.to_string());
    if let Some(kind) = barre {
        out.xml.tag_empty("barre", &[("type", kind)]);
    }
    out.xml.end_element();
}
