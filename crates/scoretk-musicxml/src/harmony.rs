// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`
// (`MusicXmlParserPass2::harmony`, `figure`, `figuredBass`) de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Harmonies et chiffrages figurés.
//!
//! Portage de `harmony` et `figured-bass` de la deuxième passe. Le modèle ne
//! garde pas la description MuseScore complète : l'accord est stocké par son
//! nom (`Harmony::name`) et sa fondamentale (`tpc`), que l'export réanalyse
//! (`export/harmony.rs`). L'import reconstruit donc le nom à partir de
//! `<kind text>` et des `<degree>`.

use scoretk_core::model::{
    Annotation, FiguredBass, FiguredBassContLine, FiguredBassItem, FiguredBassModifier, Fraction,
    Harmony, Placement, Score, SegmentType, Track,
};

use crate::pass2::{self, tpc_from_step};
use crate::xml::Element;

/// Lit un `<harmony>` et l'ajoute au segment du tick.
pub(crate) fn read_harmony(
    score: &mut Score,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    measure_start: Fraction,
    m_time: Fraction,
) {
    let divisions = score.division.max(1);
    let mut offset = Fraction::ZERO;
    let mut staff_position = 0usize;
    let mut root_tpc = scoretk_core::model::tpc::INVALID;
    let mut bass_tpc = None;
    let mut kind = String::new();
    let mut kind_text = String::new();
    let mut numeral_root: Option<String> = None;
    let mut numeral_text: Option<String> = None;
    let mut function: Option<String> = None;
    let mut degree_texts: Vec<String> = Vec::new();
    let mut placement = Placement::Above;

    if element.attribute("placement") == Some("below") {
        placement = Placement::Below;
    }

    for child in &element.children {
        match child.name.as_str() {
            "root" => {
                let step = child.child_text("root-step").unwrap_or("C");
                let alter = child.child_int("root-alter").unwrap_or(0) as i32;
                if child
                    .child("root-step")
                    .is_some_and(|step| step.attribute("text") == Some(""))
                {
                    root_tpc = scoretk_core::model::tpc::INVALID;
                } else if let Some(index) = "CDEFGAB".find(step.chars().next().unwrap_or('C')) {
                    root_tpc = tpc_from_step(index as u8, alter);
                }
            }
            "function" => {
                function = Some(child.text_trimmed().to_owned());
            }
            "numeral" => {
                if let Some(root) = child.child("numeral-root") {
                    numeral_root = Some(root.text_trimmed().to_owned());
                    numeral_text = root.attribute("text").map(str::to_owned);
                }
            }
            "kind" => {
                kind_text = child.attribute("text").unwrap_or_default().to_owned();
                kind = child.text_trimmed().to_owned();
            }
            "bass" => {
                let step = child.child_text("bass-step").unwrap_or("C");
                let alter = child.child_int("bass-alter").unwrap_or(0) as i32;
                if let Some(index) = "CDEFGAB".find(step.chars().next().unwrap_or('C')) {
                    bass_tpc = Some(tpc_from_step(index as u8, alter));
                }
            }
            "degree" => {
                let value = child.child_int("degree-value").unwrap_or(0);
                let alter = child.child_int("degree-alter").unwrap_or(0);
                let kind = child.child_text("degree-type").unwrap_or("add");
                if (1..=13).contains(&value) && (-2..=2).contains(&alter) {
                    let kind = match kind {
                        "subtract" => "sub",
                        _ => "add",
                    };
                    let alter = match alter {
                        -1 => "b",
                        1 => "#",
                        _ => "",
                    };
                    degree_texts.push(format!("{kind}{alter}{value}"));
                }
            }
            "offset" => {
                if let Ok(value) = child.text_trimmed().parse::<i64>() {
                    offset = Fraction::from_ticks(value, divisions * 4);
                }
            }
            "staff" => {
                staff_position = child
                    .text_trimmed()
                    .parse::<usize>()
                    .ok()
                    .map_or(0, |number| number.saturating_sub(1));
            }
            _ => {}
        }
    }

    let mut harmony_kind = "normal".to_owned();
    let mut name = kind_text.clone();
    let mut nashville_function: Option<String> = None;
    if numeral_root.is_some() || numeral_text.is_some() {
        let text = numeral_text.unwrap_or_default();
        let nashville = text.is_empty()
            || text
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit());
        if nashville {
            harmony_kind = "nashville".to_owned();
            name = if !kind_text.is_empty() {
                kind_text.clone()
            } else {
                kind_suffix(&kind)
            };
            nashville_function = numeral_root;
        } else {
            harmony_kind = "roman".to_owned();
            name = text;
        }
    } else if let Some(value) = function {
        // `<function>` sans `<numeral>` : le nom porte la fonction, l'export
        // la réécrit telle quelle.
        harmony_kind = "nashville".to_owned();
        name = value;
    } else if kind == "none" {
        harmony_kind = "normal".to_owned();
        name = kind_text;
    } else if kind_text.is_empty() {
        name = kind_suffix(&kind);
    }

    // Degrés ajoutés : `(add9)`, `(add#5)`…
    if !degree_texts.is_empty() {
        name.push('(');
        name.push_str(&degree_texts.join(","));
        name.push(')');
    }

    let mut harmony = Harmony::new(name.clone());
    harmony.name = name;
    harmony.root_tpc = root_tpc;
    harmony.bass_tpc = bass_tpc;
    harmony.kind = harmony_kind;
    harmony.function = nashville_function;
    harmony.placement = placement;

    let Some(track_staff) = score.parts[part_index].staves.get(staff_position).copied() else {
        return;
    };
    let track = Track::new(track_staff, 0);
    let tick = measure_start + m_time + offset;
    let index = pass2::get_segment(score, measure_index, SegmentType::ChordRest, tick);
    score.measures[measure_index].segments[index]
        .add_annotation(track, Annotation::Harmony(harmony));
}

/// Suffixe MuseScore approché d'une qualité MusicXML sans attribut `text`.
fn kind_suffix(kind: &str) -> String {
    match kind {
        "" | "major" => String::new(),
        "minor" => "m".to_owned(),
        "dominant" => "7".to_owned(),
        "major-seventh" => "M7".to_owned(),
        "minor-seventh" => "m7".to_owned(),
        "diminished" => "dim".to_owned(),
        "augmented" => "aug".to_owned(),
        "suspended-fourth" => "sus4".to_owned(),
        "suspended-second" => "sus2".to_owned(),
        "power" => "5".to_owned(),
        other => other.to_owned(),
    }
}

/// Lit un `<figured-bass>` (`figure`, `figuredBass`).
pub(crate) fn read_figured_bass(element: &Element, divisions: u32) -> FiguredBass {
    let mut bass = FiguredBass::new();
    bass.place_above = element.attribute("placement") == Some("above");
    bass.parentheses = element.attribute("parentheses") == Some("yes");
    bass.visible = element.attribute("print-object") != Some("no");
    for figure in element.children_named("figure") {
        let mut item = FiguredBassItem::default();
        if let Some(prefix) = figure.child_text("prefix") {
            item.prefix = Some(modifier(prefix));
        }
        if let Some(number) = figure.child_int("figure-number") {
            item.digit = u8::try_from(number).ok();
        }
        if let Some(suffix) = figure.child_text("suffix") {
            item.suffix = Some(modifier(suffix));
        }
        if let Some(extend) = figure.child("extend") {
            item.cont_line = match extend.attribute("type") {
                Some("start") | Some("continue") => FiguredBassContLine::Extended,
                Some("stop") => FiguredBassContLine::Simple,
                _ => FiguredBassContLine::None,
            };
        }
        bass.items.push(item);
    }
    if let Some(duration) = element.child_int("duration") {
        bass.ticks = Fraction::from_ticks(duration, divisions.max(1) * 4);
    }
    bass
}

/// Modificateur MusicXML.
fn modifier(name: &str) -> FiguredBassModifier {
    match name {
        "flat-flat" => FiguredBassModifier::DoubleFlat,
        "flat" => FiguredBassModifier::Flat,
        "natural" => FiguredBassModifier::Natural,
        "sharp" => FiguredBassModifier::Sharp,
        "double-sharp" | "sharp-sharp" => FiguredBassModifier::DoubleSharp,
        "cross" => FiguredBassModifier::Cross,
        "backslash" => FiguredBassModifier::Backslash,
        "slash" => FiguredBassModifier::Slash,
        _ => FiguredBassModifier::None,
    }
}
