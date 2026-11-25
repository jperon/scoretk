// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiexporter.cpp` (`writeHeader`,
// `writeScoreDef`, `writeStaffGrpStart`, `writeStaffDef`, `writeLabel`,
// `writeInstrDef`) de MuseScore Studio 4.7.4, Copyright (C) MuseScore Limited
// and others, modifié (réécrit en Rust) en 2026.
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

//! Header et `scoreDef` de l'écrivain MEI.

use scoretk_core::model::Score;
use scoretk_mei_common::{convert, ids};
use scoretk_xml_common::XmlWriter;

/// `MeiExporter::writeHeader` (`meiexporter.cpp:198`).
pub(crate) fn write_header(xml: &mut XmlWriter, score: &Score, date: &str) {
    xml.start_element("meiHead", &[]);
    xml.start_element("fileDesc", &[]);
    xml.start_element("titleStmt", &[]);
    xml.tag("title", &score.metadata.work_title);
    if !score.metadata.subtitle.is_empty() {
        xml.tag_with(
            "title",
            &[("type", "subordinate")],
            &score.metadata.subtitle,
        );
    }
    for (role, value) in [
        ("composer", &score.metadata.composer),
        ("lyricist", &score.metadata.lyricist),
        ("arranger", &score.metadata.arranger),
        ("translator", &score.metadata.translator),
    ] {
        if !value.is_empty() {
            xml.start_element("respStmt", &[]);
            xml.tag_with("persName", &[("role", role)], value);
            xml.end_element();
        }
    }
    xml.end_element();
    xml.start_element("pubStmt", &[]);
    if !score.metadata.copyright.is_empty() {
        xml.tag("availability", &score.metadata.copyright);
    }
    xml.tag_empty("date", &[("isodate", date)]);
    xml.end_element();
    xml.end_element();
    xml.end_element();
}

/// `MeiExporter::writeScoreDef` (`meiexporter.cpp:324`).
pub(crate) fn write_score_def(xml: &mut XmlWriter, score: &Score) {
    xml.start_element("scoreDef", &[]);
    xml.start_element("staffGrp", &[]);
    for (index, part) in score.parts.iter().enumerate() {
        if !part.long_name.is_empty() || !part.short_name.is_empty() {
            xml.start_element("staffGrp", &[]);
            xml.tag("label", &part.long_name);
            if !part.short_name.is_empty() {
                xml.tag("labelAbbr", &part.short_name);
            }
            if let Some(program) = part.instrument.midi_program {
                xml.tag_empty("instrDef", &[("midi.instrnum", &program.to_string())]);
            }
        }
        for staff_id in &part.staves {
            let staff = &score.staves[staff_id.index()];
            let (numerator, denominator, kind) = first_time_sig(score);
            let (count, unit, sym) = convert::meter_to_mei(&scoretk_core::model::TimeSig {
                numerator,
                denominator,
                kind,
                ..scoretk_core::model::TimeSig::new(1, 1)
            });
            let mut attrs = vec![
                ("n".to_owned(), (staff_id.index() + 1).to_string()),
                ("lines".to_owned(), staff.lines.to_string()),
            ];
            if let Some(count) = count {
                attrs.push(("meter.count".to_owned(), count));
            }
            if let Some(unit) = unit {
                attrs.push(("meter.unit".to_owned(), unit.to_string()));
            }
            if let Some(sym) = sym {
                attrs.push(("meter.sym".to_owned(), sym.to_owned()));
            }
            let refs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            xml.start_element("staffDef", &refs);
            let clef_type = staff.default_clef.concert;
            let (shape, line, dis, place) = convert::clef_to_mei(clef_type);
            let line_owned = line.to_string();
            let mut clef_attrs = vec![("shape", shape), ("line", &line_owned)];
            if let Some(dis) = dis {
                clef_attrs.push(("dis", dis));
            }
            if let Some(place) = place {
                clef_attrs.push(("dis.place", place));
            }
            xml.tag_empty("clef", &clef_attrs);
            xml.end_element();
            let _ = index;
        }
        if !part.long_name.is_empty() || !part.short_name.is_empty() {
            xml.end_element();
        }
    }
    xml.end_element();
    xml.end_element();
}

/// First time signature of the score, used in `scoreDef`.
fn first_time_sig(score: &Score) -> (i32, i32, scoretk_core::model::TimeSigType) {
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let scoretk_core::model::SegmentItem::TimeSig(time) = item {
                    return (time.numerator, time.denominator, time.kind);
                }
            }
        }
    }
    (4, 4, scoretk_core::model::TimeSigType::Normal)
}

/// Namespace declaration of the MEI root, as written by MuseScore.
pub(crate) fn mei_start(xml: &mut XmlWriter, date: &str) {
    xml.start_document();
    xml.processing_instruction(
        "xml-model href=\"https://music-encoding.org/schema/5.1/mei-basic.rng\" \
         type=\"application/xml\" schematypens=\"http://relaxng.org/ns/structure/1.0\"",
    );
    xml.processing_instruction(
        "xml-model href=\"https://music-encoding.org/schema/5.1/mei-basic.rng\" \
         type=\"application/xml\" schematypens=\"http://purl.oclc.org/dsdl/schematron\"",
    );
    let _ = date;
    xml.start_element(
        "mei",
        &[
            ("xmlns", ids::MEI_NAMESPACE),
            ("meiversion", ids::MEI_VERSION),
        ],
    );
}
