// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read400/connectorinforeader.cpp` et
// `engraving/rw/read400/tread.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Lignes de texte (`TextLine`) et textes de cadres `TBox` en 4.x.
//!
//! Portage de `ConnectorInfoReader` (`engraving/rw/read400/connectorinforeader.cpp`)
//! pour les `<Spanner type="TextLine">` et de `TRead::read(TBox*)`
//! (`engraving/rw/read400/tread.cpp`).
//!
//! Une ligne de texte s'écrit en deux moitiés `<Spanner type="TextLine">`,
//! chacune portant son extrémité et l'autre dans `<prev>`/`<next>`. Les moitiés
//! sont appariées en fin de lecture, comme les voltas.

use super::dom::Element;
use super::{placement, Reader};
use crate::model::{
    Fraction, HookType, LineFont, RichText, Spanner, SpannerAnchor, SpannerType, TboxText,
    TextLineInfo, TextLineStyle, Tick, Track, VOICES,
};

/// `<location>` relative d'une extrémité de spanner.
#[derive(Clone, Copy, Default)]
struct Location {
    measures: i64,
    fractions: Fraction,
    staves: i64,
    voices: i64,
}

/// Moitié de `<Spanner type="TextLine">` en attente d'appariement.
pub(super) struct TextLineHalf {
    tick: Tick,
    measure: usize,
    track: Track,
    prev: Option<Location>,
    next: Option<Location>,
    info: Option<TextLineInfo>,
}

impl TextLineHalf {
    /// Position absolue d'une extrémité, comme `Location::toAbsolute`.
    fn absolute(&self, location: Location, measures: &[crate::model::Measure]) -> (Tick, Track) {
        let target = (self.measure as i64 + location.measures).max(0) as usize;
        let (base, current_base) = match (measures.get(target), measures.get(self.measure)) {
            (Some(target), Some(current)) => (target.start_tick, current.start_tick),
            _ => (Fraction::ZERO, Fraction::ZERO),
        };
        let tick = base + (self.tick - current_base) + location.fractions;
        let raw_track =
            i64::from(self.track.raw()) + location.voices + location.staves * VOICES as i64;
        (tick, Track::from_raw(raw_track.max(0) as u32))
    }
}

/// Lit un `<Spanner type="TextLine">` rencontré dans une voix.
pub(super) fn read_spanner(
    reader: &mut Reader,
    element: &Element,
    tick: Tick,
    track: Track,
    measure_index: usize,
) {
    if element.attr("type") != Some("TextLine") {
        return;
    }

    let mut half = TextLineHalf {
        tick,
        measure: measure_index,
        track,
        prev: None,
        next: None,
        info: None,
    };
    for child in &element.children {
        match child.name.as_str() {
            "prev" => half.prev = child.child("location").map(read_location),
            "next" => half.next = child.child("location").map(read_location),
            "TextLine" => half.info = Some(read_text_line(child)),
            _ => {}
        }
    }
    reader.text_lines.push(half);
}

/// Lit les propriétés d'une `<TextLine>` (`TRead::read(TextLineBase*)`).
fn read_text_line(element: &Element) -> TextLineInfo {
    let mut info = TextLineInfo {
        system: element
            .attr("system")
            .is_some_and(|value| value != "0" && !value.is_empty()),
        placement: placement(element.child("placement")),
        ..TextLineInfo::default()
    };
    for child in &element.children {
        match child.name.as_str() {
            "lineVisible" => info.line_visible = child.int().unwrap_or(0) != 0,
            "lineStyle" => {
                info.line_style = TextLineStyle::from_mcsx(child.int().unwrap_or(0) as i32);
            }
            "beginHookType" => {
                info.begin_hook = HookType::from_mcsx(child.int().unwrap_or(0) as i32);
            }
            "endHookType" => {
                info.end_hook = HookType::from_mcsx(child.int().unwrap_or(0) as i32);
            }
            "beginHookHeight" => {
                if let Ok(value) = child.text().parse::<f64>() {
                    info.begin_hook_height = value;
                }
            }
            "endHookHeight" => {
                if let Ok(value) = child.text().parse::<f64>() {
                    info.end_hook_height = value;
                }
            }
            "beginText" => {
                let text = rich_text(child);
                if !text.is_empty() {
                    info.begin_text = Some(text);
                }
            }
            "endText" => {
                let text = rich_text(child);
                if !text.is_empty() {
                    info.end_text = Some(text);
                }
            }
            "beginFontFace" => info.begin_font.family = child.text().to_owned(),
            "endFontFace" => info.end_font.family = child.text().to_owned(),
            "beginFontSize" => {
                if let Ok(value) = child.text().parse::<f64>() {
                    info.begin_font.size = value;
                }
            }
            "endFontSize" => {
                if let Ok(value) = child.text().parse::<f64>() {
                    info.end_font.size = value;
                }
            }
            "beginFontStyle" => {
                apply_font_style(&mut info.begin_font, child.int().unwrap_or(0) as i32);
            }
            "endFontStyle" => {
                apply_font_style(&mut info.end_font, child.int().unwrap_or(0) as i32);
            }
            _ => {}
        }
    }
    info
}

/// Applique un `FontStyle` (bits `Bold`, `Italic`, `Underline`, `Strike`).
fn apply_font_style(font: &mut LineFont, style: i32) {
    font.bold = style & 1 != 0;
    font.italic = style & 2 != 0;
    font.underline = style & 4 != 0;
    font.strike = style & 8 != 0;
}

/// Texte brut d'une propriété `beginText`/`endText` (`readXml`).
fn rich_text(element: &Element) -> RichText {
    RichText::from_markup(&element.raw_inner)
}

fn read_location(element: &Element) -> Location {
    let mut location = Location::default();
    for child in &element.children {
        match child.name.as_str() {
            "measures" => location.measures = child.int().unwrap_or(0),
            "fractions" => location.fractions = child.fraction().unwrap_or(Fraction::ZERO),
            "staves" => location.staves = child.int().unwrap_or(0),
            "voices" => location.voices = child.int().unwrap_or(0),
            _ => {}
        }
    }
    location
}

/// Apparie les moitiés de lignes de texte et les ajoute à la partition.
pub(super) fn resolve(reader: &mut Reader) {
    let halves = std::mem::take(&mut reader.text_lines);
    let measures = &reader.score.measures;
    let mut spanners: Vec<Spanner> = Vec::new();

    for (index, first) in halves.iter().enumerate() {
        let Some(next) = first.next else {
            continue;
        };
        let (next_tick, next_track) = first.absolute(next, measures);
        for (other_index, second) in halves.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let Some(prev) = second.prev else {
                continue;
            };
            let (prev_tick, prev_track) = second.absolute(prev, measures);
            if prev_tick != first.tick || prev_track != first.track {
                continue;
            }
            if next_tick != second.tick || next_track != second.track {
                continue;
            }
            let info = first.info.clone().or_else(|| second.info.clone());
            let mut spanner = Spanner::new(
                SpannerType::TextLine,
                SpannerAnchor::new(first.tick, first.track),
                SpannerAnchor::new(second.tick, second.track),
            );
            if let Some(info) = info {
                spanner.text = info.begin_text.as_ref().map(|text| text.plain().to_owned());
                spanner.text_line = Some(info);
            }
            spanners.push(spanner);
            break;
        }
    }

    reader.score.spanners.extend(spanners);
}

/// Lit les paramètres de swing d'un `<SystemText>`
/// (`TRead::read(StaffTextBase*)`, balise `<swing unit="…" ratio="…"/>`).
pub(super) fn read_swing(element: &Element) -> Option<crate::model::SwingParameters> {
    let swing = element.child("swing")?;
    let unit = match swing.attr("unit") {
        Some("eighth") => crate::model::SwingUnit::Eighth,
        Some("16th") => crate::model::SwingUnit::Sixteenth,
        _ => crate::model::SwingUnit::Straight,
    };
    let ratio = swing
        .attr("ratio")
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(60);
    Some(crate::model::SwingParameters { unit, ratio })
}

/// Lit le type d'un tempo (`<type>aTempo</type>` ou `<type>tempoPrimo</type>`).
pub(super) fn read_tempo_kind(element: &Element) -> crate::model::TempoKind {
    match element.child("type").map(Element::text) {
        Some("aTempo") => crate::model::TempoKind::ATempo,
        Some("tempoPrimo") => crate::model::TempoKind::TempoPrimo,
        _ => crate::model::TempoKind::Normal,
    }
}

/// Lit le texte d'un cadre `TBox` (`TBox::text()`).
///
/// MuseScore l'exporte en `<words>` au début de la mesure qui suit le cadre
/// (`tboxTextAsWords`) ; la mesure de rattachement est l'indice courant.
pub(super) fn read_tbox(element: &Element, measure_index: usize, below: bool) -> Option<TboxText> {
    for text_element in element.children_named("Text") {
        let markup = text_element
            .child("text")
            .map_or("", |node| node.raw_inner.as_str());
        let text = RichText::from_markup(markup);
        if text.is_empty() {
            continue;
        }
        // `tboxTextAsWords` place le texte au-dessus si sa position relative
        // est négative, au-dessous sinon : un décalage vertical positif range
        // le cadre après la mesure précédente, comme un saut de page.
        let offset_below = text_element.child("offset").is_some_and(|offset| {
            offset
                .attr("y")
                .and_then(|value| value.trim().parse::<f64>().ok())
                .is_some_and(|y| y > 0.0)
        });
        let below = below || offset_below;
        let target = if below {
            measure_index.saturating_sub(1)
        } else {
            measure_index
        };
        let mut tbox = TboxText::new(text, target);
        if below {
            tbox.placement = crate::model::Placement::Below;
        }
        tbox.font_size = text_element
            .child("size")
            .and_then(|node| node.text().parse::<f64>().ok());
        return Some(tbox);
    }
    None
}
