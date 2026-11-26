// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiimporter.cpp` de MuseScore Studio
// 4.7.4 (`readMeiHead`, `readPgHead`, `readScoreDef`, `readStaffDefs`,
// `readStaffGrps`, `buildScoreParts`, `readInstrDef`), Copyright (C) MuseScore
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

//! Header, score definition and part construction.

use scoretk_core::model::{
    Clef, ClefType, Credit, CreditStyle, Fraction, Layers, Part, RichText, Staff, StaffBracket,
    Transpose,
};
use scoretk_mei_common::convert;
use scoretk_xml_common::Element;

use crate::reader::{descendants_named, Reader};

/// The sentinel used by MuseScore for global `scoreDef` values
/// (`SCOREDEF_IDX` in `meiimporter.cpp`).
const SCOREDEF_IDX: usize = usize::MAX;

impl<'a> Reader<'a> {
    /// `MeiImporter::readMeiHead` (`meiimporter.cpp:933`).
    pub(crate) fn read_mei_head(&mut self) {
        // Avec `mei-all_anyStart.rng`, la racine peut être `<meiHead>` :
        // dans ce cas c'est elle qui porte le header, pas un enfant de `<mei>`.
        let head = if self.root.name == "meiHead" {
            Some(self.root)
        } else {
            self.root.child("meiHead")
        };
        let Some(head) = head else {
            return;
        };
        let Some(file_desc) = head.child("fileDesc") else {
            return;
        };
        let Some(title_stmt) = file_desc.child("titleStmt") else {
            return;
        };
        let mut first = true;
        for title in title_stmt.children_named("title") {
            let additive = title.attribute("type") == Some("work-number");
            let text = if additive {
                crate::lines_of_additive(title)
            } else {
                title.text_trimmed().to_owned()
            };
            if text.is_empty() {
                continue;
            }
            if additive {
                self.score.metadata.work_number = text;
            } else if first {
                self.score.metadata.work_title = text.to_owned();
                first = false;
            } else if title.attribute("type") == Some("subordinate") {
                self.score.metadata.subtitle = text.to_owned();
            }
        }
        for person in descendants_named(title_stmt, "persName") {
            let role = person.attribute("role").unwrap_or_default();
            let text = person.text_trimmed();
            match role {
                "composer" => self.score.metadata.composer = text.to_owned(),
                "lyricist" => self.score.metadata.lyricist = text.to_owned(),
                "arranger" => self.score.metadata.arranger = text.to_owned(),
                "translator" => self.score.metadata.translator = text.to_owned(),
                _ => {}
            }
        }
        if let Some(text) = title_stmt.child_text("composer").filter(|t| !t.is_empty()) {
            self.score.metadata.composer = text.to_owned();
        }
        if let Some(text) = title_stmt.child_text("lyricist").filter(|t| !t.is_empty()) {
            self.score.metadata.lyricist = text.to_owned();
        }
        if let Some(text) = title_stmt.child_text("arranger").filter(|t| !t.is_empty()) {
            self.score.metadata.arranger = text.to_owned();
        }
        if let Some(pub_stmt) = file_desc.child("pubStmt") {
            if let Some(date) = pub_stmt.child("date") {
                if let Some(iso) = date.attribute("isodate") {
                    self.score.metadata.creation_date = iso.to_owned();
                }
            }
            for availability in pub_stmt.children_named("availability") {
                let text = crate::lines_of(availability);
                if !text.is_empty() {
                    self.score.metadata.copyright = text;
                }
            }
        }
        // MuseScore keeps the whole header verbatim; the pivot has no L6 slot
        // for it yet, so record the loss rather than silently dropping it.
        if head.children_named("encodingDesc").next().is_some()
            || head.children_named("sourceDesc").next().is_some()
            || head.children_named("revisionDesc").next().is_some()
        {
            self.score
                .report
                .record(Layers::EXTENSIONS, "mei-head-verbatim", None, None);
        }
    }

    /// `MeiImporter::readScoreDef` (`meiimporter.cpp:1052`).
    pub(crate) fn read_score_def(&mut self, node: &Element, is_initial: bool) {
        // MuseScore's `readScoreDef` (meiimporter.cpp:1052) only touches
        // `m_timeSigs`/`m_keySigs` when the corresponding attributes are
        // present. A mid-score `<scoreDef>` carrying only page layout must not
        // re-emit the signatures in the following measure: `Hummel_Concerto`
        // has 104 of those and the round trip gained 686 spurious time-sigs.
        let signatures_before = (self.timesigs.clone(), self.keysigs.clone());
        let count = node.attribute("meter.count");
        let unit = node
            .attribute("meter.unit")
            .and_then(|value| value.parse::<i32>().ok());
        let sym = node.attribute("meter.sym");
        if count.is_some() || sym.is_some() {
            let (fraction, kind) = convert::meter_from_mei(count, unit, sym);
            let numerator = count
                .and_then(|value| value.split('+').next())
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(fraction.numerator() as i32);
            let denominator = unit.unwrap_or(fraction.denominator() as i32);
            self.timesigs
                .insert(SCOREDEF_IDX, (numerator, denominator, kind));
            self.current_time = fraction;
            self.signatures_pending = true;
        }
        if let Some(keysig) = node.attribute("keysig") {
            if let Some(key) = convert::key_from_mei(keysig) {
                self.keysigs.insert(SCOREDEF_IDX, key);
                // `readMeiHead`/`readScoreDef` posent l'armure initiale sur
                // chaque `<staffDef>` ; un `<scoreDef keysig>` en cours de
                // partition s'applique à toutes les portées (MEI 5.1
                // `att.scoreDef.log/@keysig`) et doit remplacer ces entrées,
                // sinon la nouvelle armure n'est jamais écrite dans une mesure.
                if !is_initial {
                    for staff in 0..self.score.staves.len() {
                        self.keysigs.insert(staff, key);
                    }
                }
            }
        }
        if is_initial {
            self.read_style(node);
            self.read_pg_head(node.child("pgHead"));
            self.build_score_parts(node);
        }
        self.read_staff_defs(node);
        if let Some((numerator, denominator, _)) = self.timesigs.get(&SCOREDEF_IDX).copied() {
            self.current_time = Fraction::try_new(i64::from(numerator), i64::from(denominator))
                .unwrap_or(Fraction::new(4, 4));
        }
        if is_initial || signatures_before.0 != self.timesigs || signatures_before.1 != self.keysigs
        {
            self.signatures_pending = true;
        }
    }

    /// Page geometry from `att.pages` / `@vu.height` on `<scoreDef>`
    /// (`MEI.visual.xml`, `att.scoreDef.vis`). MEI has a single margin set, so
    /// it applies to both even and odd pages.
    fn read_style(&mut self, node: &Element) {
        let style = &mut self.score.engraving.style;
        if let Some(value) = node
            .attribute("page.width")
            .and_then(convert::measurement_to_inches)
        {
            style.page_width_in = value;
        }
        if let Some(value) = node
            .attribute("page.height")
            .and_then(convert::measurement_to_inches)
        {
            style.page_height_in = value;
        }
        for (attribute, set_even, set_odd) in [
            ("page.leftmar", 0, 1),
            ("page.rightmar", 2, 3),
            ("page.topmar", 4, 5),
            ("page.botmar", 6, 7),
        ] {
            let Some(value) = node
                .attribute(attribute)
                .and_then(convert::measurement_to_inches)
            else {
                continue;
            };
            let margins = [
                &mut style.page_even_left_margin_in,
                &mut style.page_odd_left_margin_in,
                &mut style.page_even_right_margin_in,
                &mut style.page_odd_right_margin_in,
                &mut style.page_even_top_margin_in,
                &mut style.page_odd_top_margin_in,
                &mut style.page_even_bottom_margin_in,
                &mut style.page_odd_bottom_margin_in,
            ];
            *margins[set_even] = value;
            *margins[set_odd] = value;
        }
        if let Some(value) = node
            .attribute("vu.height")
            .and_then(convert::measurement_to_mm)
        {
            style.spatium_mm = value;
        }
        // `att.optimization/@optimize` is on `att.scoreDef.vis` as well.
        if let Some(value) = node.attribute("optimize") {
            style.hide_empty_staves = matches!(value, "true" | "1");
        }
    }

    /// `MeiImporter::readPgHead` (`meiimporter.cpp:1102`): the first-page
    /// header becomes the title credits when the `meiHead` has no title.
    fn read_pg_head(&mut self, node: Option<&Element>) {
        let Some(node) = node else {
            return;
        };
        // MuseScore turns the first-page head into a title VBox; the pivot's
        // closest equivalent is an ordered list of credits. `@n` is the
        // additive carrier of the model order (MuseScore groups the texts by
        // alignment cell, which reorders them).
        let mut numbered: Vec<(usize, Credit)> = Vec::new();
        for rend in descendants_named(node, "rend") {
            let Some(style_name) = rend.attribute("type") else {
                continue;
            };
            let credit_style = CreditStyle::from_name(style_name);
            // Numeric `@fontsize` in points is standard MEI
            // (`data.FONTSIZENUMERIC`); relative terms carry no size the pivot
            // can store. An empty `<rend>` still carries a text-style size.
            if let Some(size) = rend
                .attribute("fontsize")
                .and_then(convert::font_size_to_points)
            {
                if rend.attribute("n").is_none() {
                    let style = &mut self.score.engraving.style;
                    match credit_style {
                        CreditStyle::Title => style.title_font_size = size,
                        CreditStyle::Composer => style.composer_font_size = size,
                        CreditStyle::Lyricist => style.lyricist_font_size = size,
                        _ => {}
                    }
                }
            }
            let text = if rend.attribute("n").is_some() {
                crate::lines_of_additive(rend)
            } else {
                crate::lines_of(rend)
            };
            if text.is_empty() {
                continue;
            }
            let mut credit = Credit::new(RichText::new(text), credit_style);
            if let Some(align) = rend.attribute("halign") {
                if let Some(justify) = scoretk_core::model::CreditJustify::from_name(align) {
                    credit.justify = justify;
                }
            }
            if let Some(align) = rend.attribute("valign") {
                if let Some(valign) = scoretk_core::model::CreditValign::from_name(align) {
                    credit.valign = valign;
                }
            }
            // La taille *explicite* d'un credit n'est portée que par la
            // variante additive (`@n`) : dans le format 4.7.4, `@fontsize`
            // donne la taille du style, pas celle du texte.
            if rend.attribute("n").is_some() {
                credit.font_size = rend
                    .attribute("fontsize")
                    .and_then(convert::font_size_to_points);
            }
            if let Some(frame) = rend
                .attribute("label")
                .and_then(|label| label.strip_prefix(scoretk_mei_common::ids::CREDIT_FRAME_LABEL))
            {
                let mut fields = frame.split(':');
                if let Some(page) = fields.next().and_then(|value| value.parse::<u32>().ok()) {
                    credit.page = page;
                }
                if let Some(height) = fields.next().and_then(|value| value.parse::<f64>().ok()) {
                    credit.vbox_height = height;
                }
            }
            match rend
                .attribute("n")
                .and_then(|value| value.parse::<usize>().ok())
            {
                Some(n) => numbered.push((n, credit)),
                None => {
                    self.score.add_credit(credit);
                }
            }
        }
        numbered.sort_by_key(|(n, _)| *n);
        for (_, credit) in numbered {
            self.score.add_credit(credit);
        }
        if self.score.metadata.work_title.is_empty() {
            if let Some(title) = descendants_named(node, "title").first() {
                let text = crate::lines_of(title);
                if !text.is_empty() {
                    self.score.metadata.work_title = text;
                }
            }
        }
        if self.score.metadata.composer.is_empty() {
            if let Some(composer) = descendants_named(node, "composer").first() {
                let text = crate::lines_of(composer);
                if !text.is_empty() {
                    self.score.metadata.composer = text;
                }
            }
        }
    }

    /// `MeiImporter::buildScoreParts` (`meiimporter.cpp:3390`).
    fn build_score_parts(&mut self, score_def: &Element) {
        let staff_defs = descendants_named(score_def, "staffDef");
        if staff_defs.is_empty() {
            return;
        }
        let mut part_for_staff: std::collections::BTreeMap<i32, usize> =
            std::collections::BTreeMap::new();
        // A labelled staffGrp groups its staves into one part, as in MuseScore.
        for group in descendants_named(score_def, "staffGrp") {
            let label = group.child_text("label").unwrap_or_default();
            let abbr = group.child_text("labelAbbr").unwrap_or_default();
            let children = descendants_named(group, "staffDef");
            if label.is_empty() || children.is_empty() {
                continue;
            }
            let mut part = Part::new(label, abbr);
            if let Some(id) = scoretk_core::model::recognize_musicxml_id(&part.long_name, None) {
                part.instrument.id = id.to_owned();
            }
            let part_id = self.score.add_part(part);
            let part_index = part_id.index();
            for child in children {
                if let Some(n) = child
                    .attribute("n")
                    .and_then(|value| value.parse::<i32>().ok())
                {
                    part_for_staff.insert(n, part_index);
                }
            }
            if let Some(instr) = group.child("instrDef") {
                self.read_instr_def(instr, part_id);
            }
        }
        for staff_def in &staff_defs {
            let n = staff_def
                .attribute("n")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(1);
            let part_index = if let Some(index) = part_for_staff.get(&n).copied() {
                index
            } else {
                let label = staff_def.child_text("label").unwrap_or_default();
                let abbr = staff_def.child_text("labelAbbr").unwrap_or_default();
                let source = staff_def.child("instrDef");
                let long_name = if label.is_empty() {
                    format!("Staff {n}")
                } else {
                    label.to_owned()
                };
                let mut part = Part::new(long_name.clone(), abbr);
                if let Some(id) = scoretk_core::model::recognize_musicxml_id(&long_name, None) {
                    part.instrument.id = id.to_owned();
                }
                let id = self.score.add_part(part);
                let index = id.index();
                if let Some(instr) = source {
                    self.read_instr_def(instr, id);
                }
                part_for_staff.insert(n, index);
                index
            };
            self.part_by_staff.insert(n, part_index);
            let part_id = scoretk_core::model::PartId::from_index(part_index);
            let mut staff = Staff::new(part_id);
            staff.lines = staff_def
                .attribute("lines")
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(5);
            staff.visible = staff_def.attribute("lines.visible") != Some("false");
            staff.user_mag_percent = staff_def
                .attribute("scale")
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(100.0) as i32;
            if let Some(clef) = staff_def.child("clef") {
                staff.default_clef = Clef::new(clef_type_of(clef));
            } else if let (Some(shape), Some(line)) = (
                staff_def.attribute("clef.shape"),
                staff_def.attribute("clef.line"),
            ) {
                staff.default_clef = Clef::new(convert::clef_from_mei(
                    Some(shape),
                    line.parse().unwrap_or(2),
                    staff_def.attribute("clef.dis"),
                    staff_def.attribute("clef.dis.place"),
                ));
            }
            let part = &mut self.score.parts[part_index];
            if part_for_staff
                .values()
                .filter(|value| **value == part_index)
                .count()
                > 1
            {
                part.instrument.track_name = part.long_name.clone();
            }
            self.score.add_staff_with(part_id, staff);
        }
        // Staff brackets read from the top-level staffGrp.
        for group in score_def.children_named("staffGrp") {
            let symbol = group
                .attribute("symbol")
                .or_else(|| {
                    group
                        .child("grpSym")
                        .and_then(|node| node.attribute("symbol"))
                })
                .unwrap_or_default();
            let kind = match symbol {
                "bracket" | "bracketsq" => 0,
                "brace" => 1,
                "line" | "bar.thru" => 2,
                _ => -1,
            };
            if kind < 0 {
                continue;
            }
            let span = descendants_named(group, "staffDef").len();
            let thru = group.attribute("bar.thru") == Some("true");
            for staff in &mut self.score.staves {
                staff.brackets.push(StaffBracket {
                    kind,
                    span,
                    visible: true,
                });
                if thru {
                    staff.bar_line_span = true;
                }
            }
        }
    }

    /// `MeiImporter::readInstrDef` (`meiimporter.cpp:2849`).
    fn read_instr_def(&mut self, node: &Element, part: scoretk_core::model::PartId) {
        let part = &mut self.score.parts[part.index()];
        if let Some(program) = node
            .attribute("midi.instrnum")
            .and_then(|value| value.parse::<u8>().ok())
        {
            part.instrument.midi_program = Some(program);
            if part.instrument.id.is_empty() {
                if let Some(id) = scoretk_core::model::recognize_musicxml_id("", Some(program)) {
                    part.instrument.id = id.to_owned();
                }
            }
        }
        // MEI 5.1 `att.channelized/@midi.channel` et
        // `att.midiInstrument/@midi.volume`/`@midi.pan`. Les attributs courts
        // `@volume`/`@pan` sont ceux de MuseScore 4.7.4 et restent lus.
        if let Some(channel) = node
            .attribute("midi.channel")
            .or_else(|| node.attribute("channel"))
            .and_then(|value| value.parse::<u8>().ok())
        {
            part.instrument.midi_channel = Some(channel);
        }
        if let Some(volume) = node
            .attribute("midi.volume")
            .or_else(|| node.attribute("volume"))
            .and_then(|value| value.parse::<u8>().ok())
        {
            part.instrument.volume = Some(volume);
        }
        if let Some(pan) = node
            .attribute("midi.pan")
            .or_else(|| node.attribute("pan"))
            .and_then(|value| value.parse::<u8>().ok())
        {
            part.instrument.pan = Some(pan);
        }
        if let Some(id) = node
            .attribute("type")
            .and_then(|value| value.strip_prefix(scoretk_mei_common::ids::INSTRUMENT_TYPE))
            .filter(|id| !id.is_empty())
        {
            part.instrument.id = id.to_owned();
        }
    }

    /// `MeiImporter::readStaffDefs` (`meiimporter.cpp:1226`).
    fn read_staff_defs(&mut self, node: &Element) {
        for staff_def in descendants_named(node, "staffDef") {
            let n = staff_def
                .attribute("n")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0);
            let staff_idx = self.staff_by_n.get(&n).copied().unwrap_or(0);
            if let Some(clef) = staff_def.child("clef") {
                self.clefs.insert(staff_idx, clef_type_of(clef));
            } else if let (Some(shape), Some(line)) = (
                staff_def.attribute("clef.shape"),
                staff_def.attribute("clef.line"),
            ) {
                self.clefs.insert(
                    staff_idx,
                    convert::clef_from_mei(
                        Some(shape),
                        line.parse().unwrap_or(2),
                        staff_def.attribute("clef.dis"),
                        staff_def.attribute("clef.dis.place"),
                    ),
                );
            }
            let count = staff_def.attribute("meter.count").or_else(|| {
                staff_def
                    .child("meterSig")
                    .and_then(|child| child.attribute("count"))
            });
            let unit = staff_def
                .attribute("meter.unit")
                .or_else(|| {
                    staff_def
                        .child("meterSig")
                        .and_then(|child| child.attribute("unit"))
                })
                .and_then(|value| value.parse::<i32>().ok());
            let sym = staff_def.attribute("meter.sym").or_else(|| {
                staff_def
                    .child("meterSig")
                    .and_then(|child| child.attribute("sym"))
            });
            if count.is_some() || sym.is_some() {
                let (_, kind) = convert::meter_from_mei(count, unit, sym);
                let numerator = count
                    .and_then(|value| value.split('+').next())
                    .and_then(|value| value.parse::<i32>().ok())
                    .unwrap_or(4);
                self.timesigs
                    .insert(staff_idx, (numerator, unit.unwrap_or(4), kind));
            }
            let keysig = staff_def.attribute("keysig").or_else(|| {
                staff_def
                    .child("keySig")
                    .and_then(|child| child.attribute("sig"))
            });
            if let Some(key) = keysig.and_then(convert::key_from_mei) {
                self.keysigs.insert(staff_idx, key);
            }
            if let (Some(diatonic), Some(chromatic)) = (
                staff_def
                    .attribute("trans.diat")
                    .and_then(|value| value.parse::<i32>().ok()),
                staff_def
                    .attribute("trans.semi")
                    .and_then(|value| value.parse::<i32>().ok()),
            ) {
                self.transpose
                    .insert(staff_idx, Transpose::new(diatonic, chromatic));
                if let Some(part_index) = self.part_by_staff.get(&n).copied() {
                    self.score.parts[part_index].instrument.transpose =
                        Some(Transpose::new(diatonic, chromatic));
                }
            }
        }
    }
}

/// `Convert::clefFromMEI(const Clef&)` for a `<clef>` child.
fn clef_type_of(node: &Element) -> ClefType {
    convert::clef_from_mei(
        node.attribute("shape"),
        node.attribute("line")
            .and_then(|value| value.parse().ok())
            .unwrap_or(2),
        node.attribute("dis"),
        node.attribute("dis.place"),
    )
}
