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

use scoretk_core::model::{CreditStyle, Part, Score, SegmentItem, SegmentType, StaffId, Track};
use scoretk_mei_common::{convert, ids};
use scoretk_xml_common::XmlWriter;

use crate::MeiOptions;

/// `MeiExporter::writeHeader` (`meiexporter.cpp:198`).
pub(crate) fn write_header(xml: &mut XmlWriter, score: &Score, date: &str, options: &MeiOptions) {
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
    // `workNumber`/`work_number` n'a pas d'élément MEI dédié : un `<title>`
    // typé est un porteur standard (`att.typed`) et reste lisible par
    // `readMeiHead` (porteur additionnel, 4.7.4 ne l'écrit pas).
    if options.full_support && !score.metadata.work_number.is_empty() {
        xml.tag_with(
            "title",
            &[("type", "work-number")],
            &score.metadata.work_number,
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
pub(crate) fn write_pg_head(xml: &mut XmlWriter, score: &Score, options: &MeiOptions) {
    if options.full_support {
        write_pg_head_ordered(xml, score);
        return;
    }
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

/// Variante additive de `writePgHead` : les texts sont écrits dans l'ordre
/// du modèle (et non regroupés par cellule d'alignement), avec `@n` pour
/// retrouver cet ordre et les attributs standard `@halign`/`@valign`/
/// `@fontsize`. Le numéro de page et la hauteur du cadre n'ont pas de
/// contrepartie MEI 5.1 : ils sont portés par `@label="mscore-frame-…"`.
fn write_pg_head_ordered(xml: &mut XmlWriter, score: &Score) {
    let style = &score.engraving.style;
    if score.engraving.credits.is_empty() {
        write_pg_head(xml, score, &MeiOptions::default());
        return;
    }
    xml.start_element("pgHead", &[("func", "first")]);
    for (index, credit) in score.engraving.credits.iter().enumerate() {
        let (_, name, term) = convert::text_to_mei(&credit.style);
        let size = credit
            .font_size
            .map(convert::points_to_mei)
            .or_else(|| term.map(str::to_owned));
        let label = format!(
            "{}{}:{:.5}",
            ids::CREDIT_FRAME_LABEL,
            credit.page,
            credit.vbox_height
        );
        let mut attrs = vec![
            ("type".to_owned(), name.to_owned()),
            ("n".to_owned(), (index + 1).to_string()),
            (
                "halign".to_owned(),
                credit.justify.musicxml_name().to_owned(),
            ),
            (
                "valign".to_owned(),
                credit.valign.musicxml_name().to_owned(),
            ),
            ("label".to_owned(), label),
        ];
        if let Some(size) = size {
            attrs.push(("fontsize".to_owned(), size));
        }
        let refs: Vec<(&str, &str)> = attrs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        xml.tag_with("rend", &refs, credit.text.plain());
    }
    // Les styles de texte peuvent être fixés sans texte correspondant dans
    // la première frame : un `<rend>` vide est légal en MEI 5.1.
    for (style_kind, size) in [
        (CreditStyle::Title, style.title_font_size),
        (CreditStyle::Composer, style.composer_font_size),
        (CreditStyle::Lyricist, style.lyricist_font_size),
    ] {
        // Écrire la taille de style même quand un credit du même style
        // existe : `@fontsize` d'un credit est une taille *explicite*, pas
        // le défaut du style, et les deux informations sont distinctes.
        let (_, name, _) = convert::text_to_mei(&style_kind);
        let size = convert::points_to_mei(size);
        xml.tag_empty("rend", &[("type", name), ("fontsize", &size)]);
    }
    xml.end_element();
}

/// `MeiExporter::writeScoreDef` (`meiexporter.cpp:324`).
pub(crate) fn write_score_def(xml: &mut XmlWriter, score: &Score, options: &MeiOptions) {
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
    write_pg_head(xml, score, options);
    xml.start_element("staffGrp", &[]);
    for (index, part) in score.parts.iter().enumerate() {
        if !part.long_name.is_empty() || !part.short_name.is_empty() {
            xml.start_element("staffGrp", &[]);
            xml.tag("label", &part.long_name);
            if !part.short_name.is_empty() {
                xml.tag("labelAbbr", &part.short_name);
            }
            let attributes = instr_def_attributes(part, options.full_support);
            if !attributes.is_empty() {
                let refs: Vec<(&str, &str)> = attributes
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect();
                xml.tag_empty("instrDef", &refs);
            }
        }
        for (staff_index, staff_id) in part.staves.iter().enumerate() {
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
            // `Convert::staffToMEI` (meiconverter.cpp:3099) porte la
            // transposition de la partie sur chaque `<staffDef>` : sans ces
            // attributs, la lecture suivante prend la hauteur écrite pour une
            // hauteur réelle et transpose une seconde fois.
            if let Some(transpose) = part.instrument.transpose {
                attrs.push(("trans.diat".to_owned(), transpose.diatonic.to_string()));
                attrs.push(("trans.semi".to_owned(), transpose.chromatic.to_string()));
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
            // Une partie sans libellé n'a pas de `<staffGrp>` portant un
            // `<instrDef>` : écrire la définition dans son premier
            // `<staffDef>` rend l'identité et les paramètres MIDI lisibles.
            if options.full_support && staff_index == 0 {
                let attributes = instr_def_attributes(part, true);
                if !attributes.is_empty() {
                    let refs: Vec<(&str, &str)> = attributes
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.as_str()))
                        .collect();
                    xml.tag_empty("instrDef", &refs);
                }
            }
            // MuseScore porte la clef initiale sur le segment de la première
            // mesure (`writeStaffDef`, meiexporter.cpp:684) ; le `default_clef`
            // de la portée reste souvent G. Prendre la clé réellement jouée.
            let clef_type = score
                .measures
                .first()
                .and_then(|measure| {
                    // Le porteur additionnel distingue la clé d'en-tête
                    // (`SegmentType::HeaderClef`, écrite dans `<staffDef>`)
                    // de la clé du premier segment `Clef` (écrite dans la
                    // couche) : les deux sont des faits distincts du modèle.
                    let track = Track::new(StaffId::from_index(staff_id.index()), 0);
                    let pick = |kind| {
                        measure.segments.iter().find_map(|segment| {
                            if segment.kind != kind || segment.tick != measure.start_tick {
                                return None;
                            }
                            match segment.element(track) {
                                Some(SegmentItem::Clef(clef)) => Some(clef.concert),
                                _ => None,
                            }
                        })
                    };
                    if options.full_support {
                        pick(SegmentType::HeaderClef).or_else(|| pick(SegmentType::Clef))
                    } else {
                        pick(SegmentType::Clef)
                    }
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

/// Attributs d'un `<instrDef>` (MEI 5.1 `att.midiInstrument`, `att.channelized`
/// et `att.typed`). Les champs numériques sont standard ; l'identifiant
/// d'instrument de MuseScore est porté par un `@type="mscore-instrument-*"`
/// documenté, `@midi.instrname` étant limité à l'énumération fermée
/// `data.MIDINAMES`. Ces attributs additionnels sont derrière
/// `MeiOptions::full_support`.
fn instr_def_attributes(part: &Part, full_support: bool) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    if let Some(program) = part.instrument.midi_program {
        attributes.push(("midi.instrnum".to_owned(), program.to_string()));
    }
    if !full_support {
        return attributes;
    }
    if let Some(channel) = part.instrument.midi_channel {
        attributes.push(("midi.channel".to_owned(), channel.to_string()));
    }
    if let Some(volume) = part.instrument.volume {
        attributes.push(("midi.volume".to_owned(), volume.to_string()));
    }
    if let Some(pan) = part.instrument.pan {
        attributes.push(("midi.pan".to_owned(), pan.to_string()));
    }
    if !part.instrument.id.is_empty() {
        attributes.push((
            "type".to_owned(),
            format!("{}{}", ids::INSTRUMENT_TYPE, part.instrument.id),
        ));
    }
    attributes
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
pub(crate) fn mei_start(xml: &mut XmlWriter, date: &str, full_support: bool) {
    xml.start_document();
    // Le mode additif écrit des éléments de `mei-all` : il annonce `mei-all`
    // et la version de la RNG normative (`meiversion="5.1"`), tandis que le
    // chemin 4.7.4 garde `mei-basic`/`5.1+basic` octet pour octet.
    let schema = if full_support { "mei-all" } else { "mei-basic" };
    for schematypens in [
        "http://relaxng.org/ns/structure/1.0",
        "http://purl.oclc.org/dsdl/schematron",
    ] {
        xml.processing_instruction(&format!(
            "xml-model href=\"https://music-encoding.org/schema/5.1/{schema}.rng\" \
             type=\"application/xml\" schematypens=\"{schematypens}\""
        ));
    }
    let _ = date;
    let version = if full_support {
        "5.1"
    } else {
        ids::MEI_VERSION
    };
    xml.start_element(
        "mei",
        &[("xmlns", ids::MEI_NAMESPACE), ("meiversion", version)],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::write;
    use scoretk_core::model::{Credit, CreditStyle, RichText};

    /// `<instrDef>` porte l'identité et les paramètres MIDI d'une partie
    /// (MEI 5.1 `att.midiInstrument`/`att.channelized`). MuseScore 4.7.4
    /// n'écrit que `@midi.instrnum` et seulement pour une partie nommée.
    #[test]
    fn instr_def_carries_instrument_identity_when_enabled() {
        use scoretk_core::model::{Fraction, Measure, Track};

        let mut score = Score::new();
        let mut part = Part::new("Voice", "Vox");
        part.instrument.id = "voice.vocals".to_owned();
        part.instrument.midi_program = Some(52);
        part.instrument.midi_channel = Some(3);
        part.instrument.volume = Some(101);
        part.instrument.pan = Some(64);
        let part_id = score.add_part(part);
        let staff = score.add_staff(part_id);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = scoretk_core::model::Segment::new(
            scoretk_core::model::SegmentType::ChordRest,
            Fraction::ZERO,
            1,
        );
        let chord = scoretk_core::model::ChordRest::chord(scoretk_core::model::Duration::quarter());
        segment.set_element(
            Track::new(staff, 0),
            scoretk_core::model::SegmentItem::ChordRest(chord),
        );
        measure.add_segment(segment);
        score.add_measure(measure);

        let options = MeiOptions {
            full_support: true,
            ..Default::default()
        };
        let output = write(&score, &options).expect("MEI with instrument data");
        assert!(output.contains("midi.instrnum=\"52\""));
        assert!(output.contains("midi.channel=\"3\""));
        assert!(output.contains("midi.volume=\"101\""));
        assert!(output.contains("midi.pan=\"64\""));
        assert!(output.contains("type=\"mscore-instrument-voice.vocals\""));
        let back = scoretk_mei_import::read_mei(output.as_bytes()).expect("read MEI");
        assert_eq!(back.parts[0].instrument.id, "voice.vocals");
        assert_eq!(back.parts[0].instrument.midi_program, Some(52));
        assert_eq!(back.parts[0].instrument.midi_channel, Some(3));
        assert_eq!(back.parts[0].instrument.volume, Some(101));
        assert_eq!(back.parts[0].instrument.pan, Some(64));
    }

    /// L'ordre des credits, leur page, leur taille et la hauteur du cadre
    /// sont conservés par les porteurs additionnels (`@n`, `@halign`,
    /// `@valign`, `@fontsize`, `@label`).
    #[test]
    fn ordered_credits_survive_an_extended_round_trip() {
        let mut score = Score::new();
        let mut title = Credit::new(RichText::new("Titre"), CreditStyle::Title);
        title.font_size = Some(22.0);
        title.vbox_height = 10.0;
        score.engraving.credits.push(title);
        let mut composer = Credit::new(RichText::new("Compositeur"), CreditStyle::Composer);
        composer.vbox_height = 18.0977;
        score.engraving.credits.push(composer);
        let mut later = Credit::new(RichText::new("Titre page 2"), CreditStyle::Title);
        later.page = 2;
        later.justify = scoretk_core::model::CreditJustify::Left;
        later.valign = scoretk_core::model::CreditValign::Top;
        score.engraving.credits.push(later);

        let output = write(
            &score,
            &MeiOptions {
                full_support: true,
                ..Default::default()
            },
        )
        .expect("extended MEI with credits");
        assert!(output.contains("n=\"1\""));
        let back = scoretk_mei_import::read_mei(output.as_bytes()).expect("read MEI");
        assert_eq!(back.engraving.credits.len(), 3);
        assert_eq!(back.engraving.credits[0].text.plain(), "Titre");
        assert_eq!(back.engraving.credits[1].text.plain(), "Compositeur");
        assert!((back.engraving.credits[1].vbox_height - 18.0977).abs() < 1e-4);
        assert_eq!(back.engraving.credits[2].text.plain(), "Titre page 2");
        assert_eq!(back.engraving.credits[2].page, 2);
        assert_eq!(
            back.engraving.credits[2].justify,
            scoretk_core::model::CreditJustify::Left
        );
        assert_eq!(back.engraving.credits[0].font_size, Some(22.0));
    }

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
        write_pg_head(&mut xml, &score, &MeiOptions::default());
        let output = xml.finish();
        assert!(output.contains("<pgHead func=\"first\">"));
        assert!(output.contains("halign=\"center\" valign=\"top\""));
        assert!(output.contains("type=\"title\" fontsize=\"22.00pt\">Title"));
        assert!(output.contains("halign=\"right\" valign=\"bottom\""));
        assert!(output.contains("type=\"composer\" fontsize=\"10.00pt\">Composer"));
        assert!(!output.contains("Later"));
    }
}
