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

use scoretk_core::model::{CreditStyle, Score, SegmentItem, SegmentType, StaffId, Track};
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

/// `MeiExporter::writePgHead` (`meiexporter.cpp:386`).
///
/// Only the first-frame texts are carried: MuseScore writes one `<pgHead>`
/// from the first `VBox`, grouping the texts into nine alignment cells.
pub(crate) fn write_pg_head(xml: &mut XmlWriter, score: &Score) {
    let mut cells: [Vec<(&str, Option<String>, &str)>; 9] = Default::default();
    let style = &score.engraving.style;
    for credit in &score.engraving.credits {
        if credit.page != 1 {
            continue;
        }
        let (cell, name, term) = convert::text_to_mei(&credit.style);
        // `@fontsize` accepts numeric point sizes in MEI 5.1
        // (`data.FONTSIZENUMERIC`); MuseScore's own exporter writes relative
        // terms. Keep the term for subtitle, carry the exact style size for
        // the three text styles whose size the pivot records.
        let fontsize = match credit.style {
            CreditStyle::Title => Some(convert::points_to_mei(style.title_font_size)),
            CreditStyle::Composer => Some(convert::points_to_mei(style.composer_font_size)),
            CreditStyle::Lyricist => Some(convert::points_to_mei(style.lyricist_font_size)),
            _ => term.map(str::to_owned),
        };
        cells[cell as usize].push((name, fontsize, credit.text.plain()));
    }
    // A text style can be set in the `.mscx` even when no text of that style
    // sits in the first frame. An empty `<rend>` is legal MEI (its content is
    // optional) and is the standard carrier for its numeric `@fontsize`.
    let present = |expected: &CreditStyle| {
        score
            .engraving
            .credits
            .iter()
            .any(|credit| credit.page == 1 && &credit.style == expected)
    };
    for (style_kind, size) in [
        (CreditStyle::Title, style.title_font_size),
        (CreditStyle::Composer, style.composer_font_size),
        (CreditStyle::Lyricist, style.lyricist_font_size),
    ] {
        if present(&style_kind) {
            continue;
        }
        let (cell, name, _) = convert::text_to_mei(&style_kind);
        cells[cell as usize].push((name, Some(convert::points_to_mei(size)), ""));
    }
    if cells.iter().all(|cell| cell.is_empty()) {
        return;
    }
    xml.start_element("pgHead", &[("func", "first")]);
    for (index, rends) in cells.iter().enumerate() {
        if rends.is_empty() {
            continue;
        }
        let (valign, halign) = convert::cell_to_align(index as u8);
        xml.start_element("rend", &[("halign", halign), ("valign", valign)]);
        for (position, (name, fontsize, text)) in rends.iter().enumerate() {
            if position > 0 {
                xml.tag_empty("lb", &[]);
            }
            let mut attrs = vec![("type", *name)];
            if let Some(fontsize) = fontsize {
                attrs.push(("fontsize", fontsize.as_str()));
            }
            xml.tag_with("rend", &attrs, text);
        }
        xml.end_element();
    }
    xml.end_element();
}

/// `MeiExporter::writeScoreDef` (`meiexporter.cpp:324`).
pub(crate) fn write_score_def(xml: &mut XmlWriter, score: &Score) {
    // `att.pages` and `att.spacing`/`@vu.height` belong to `att.scoreDef.vis`
    // (MEI 5.1, MEI.visual.xml). MEI has one margin set, so its single values
    // describe both even and odd pages.
    let style = &score.engraving.style;
    let page_attrs = [
        (
            "page.width".to_owned(),
            convert::inches_to_mei(style.page_width_in),
        ),
        (
            "page.height".to_owned(),
            convert::inches_to_mei(style.page_height_in),
        ),
        (
            "page.leftmar".to_owned(),
            convert::inches_to_mei(style.page_odd_left_margin_in),
        ),
        (
            "page.rightmar".to_owned(),
            convert::inches_to_mei(style.page_odd_right_margin_in),
        ),
        (
            "page.topmar".to_owned(),
            convert::inches_to_mei(style.page_odd_top_margin_in),
        ),
        (
            "page.botmar".to_owned(),
            convert::inches_to_mei(style.page_odd_bottom_margin_in),
        ),
        ("vu.height".to_owned(), convert::mm_to_mei(style.spatium_mm)),
    ];
    let optimize = if style.hide_empty_staves {
        Some(("optimize".to_owned(), "true".to_owned()))
    } else {
        None
    };
    let page_attrs = match optimize {
        Some(pair) => [page_attrs.to_vec(), vec![pair]].concat(),
        None => page_attrs.to_vec(),
    };
    let page_refs: Vec<(&str, &str)> = page_attrs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("scoreDef", &page_refs);
    write_pg_head(xml, score);
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
            // `Convert::staffToMEI`/`writeStaffDef` portent l'armure initiale
            // sur chaque `<staffDef>` (meiexporter.cpp:716). La clé de Do est
            // omise, comme dans `Convert::keyToMEI`.
            if let Some(key) = score
                .measures
                .first()
                .and_then(|measure| {
                    measure.segments.iter().find_map(|segment| {
                        match segment.element(Track::new(StaffId::from_index(staff_id.index()), 0))
                        {
                            Some(SegmentItem::KeySig(key))
                                if segment.tick == measure.start_tick =>
                            {
                                Some(key.concert_key)
                            }
                            _ => None,
                        }
                    })
                })
                .filter(|key| key.fifths() != 0)
            {
                attrs.push(("keysig".to_owned(), convert::key_to_mei(key)));
            }
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
            // MuseScore porte la clef initiale sur le segment de la première
            // mesure (`writeStaffDef`, meiexporter.cpp:684) ; le `default_clef`
            // de la portée reste souvent G. Prendre la clé réellement jouée.
            let clef_type = score
                .measures
                .first()
                .and_then(|measure| {
                    measure
                        .segments
                        .iter()
                        .find(|segment| {
                            segment.kind == SegmentType::Clef
                                && segment.tick == measure.start_tick
                                && segment
                                    .element(Track::new(StaffId::from_index(staff_id.index()), 0))
                                    .is_some()
                        })
                        .and_then(|segment| {
                            match segment
                                .element(Track::new(StaffId::from_index(staff_id.index()), 0))
                            {
                                Some(SegmentItem::Clef(clef)) => Some(clef.concert),
                                _ => None,
                            }
                        })
                })
                .unwrap_or(staff.default_clef.concert);
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

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Credit, CreditStyle, RichText};

    #[test]
    fn pg_head_groups_first_page_texts_into_cells() {
        let mut score = Score::new();
        score
            .engraving
            .credits
            .push(Credit::new(RichText::new("Title"), CreditStyle::Title));
        score.engraving.credits.push(Credit::new(
            RichText::new("Composer"),
            CreditStyle::Composer,
        ));
        let mut later = Credit::new(RichText::new("Later"), CreditStyle::Title);
        later.page = 2;
        score.engraving.credits.push(later);
        let mut xml = XmlWriter::new();
        write_pg_head(&mut xml, &score);
        let output = xml.finish();
        assert!(output.contains("<pgHead func=\"first\">"));
        assert!(output.contains("halign=\"center\" valign=\"top\""));
        assert!(output.contains("type=\"title\" fontsize=\"22.00pt\">Title"));
        assert!(output.contains("halign=\"right\" valign=\"bottom\""));
        assert!(output.contains("type=\"composer\" fontsize=\"10.00pt\">Composer"));
        assert!(!output.contains("Later"));
    }
}
