// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`
// (`MusicXmlParserPass2::barline`, `doEnding`) de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Barres de mesure, reprises et voltas.
//!
//! Le modèle range le style de barre dans la mesure (`start_bar_line` /
//! `end_bar_line`), les reprises dans les mêmes champs et les voltas dans les
//! spanners. L'export les réécrit dans `<barline location="left|right">`.

use scoretk_core::model::{
    Annotation, BarLine, BarLineType, Fermata, FermataType, Placement, Score, SegmentItem,
    SegmentType, Spanner, SpannerAnchor, SpannerType, Track, VoltaInfo,
};

use crate::pass2;
use crate::xml::Element;

/// Lit un `<barline>` (`MusicXmlParserPass2::barline`).
pub(crate) fn read_barline(
    score: &mut Score,
    part_index: usize,
    measure_index: usize,
    element: &Element,
) {
    let location = element.attribute("location").unwrap_or("right");
    let Some(staff_id) = score.parts[part_index].staves.first().copied() else {
        return;
    };
    let track = Track::new(staff_id, 0);
    let measure_end = score.measures[measure_index].end_tick();

    if let Some(style) = element.child_text("bar-style") {
        let kind = barline_type(style);
        if location == "left" {
            let current = score.measures[measure_index].start_bar_line;
            if !current.is_repeat_start() {
                score.measures[measure_index].start_bar_line = kind;
            }
        } else if location == "right" {
            let current = score.measures[measure_index].end_bar_line;
            if !current.is_repeat_end() {
                score.measures[measure_index].end_bar_line = kind;
            }
            // MuseScore représente aussi la barre par un élément sur le
            // segment de fin (`engraving/dom/barline.h`) ; le lecteur ne
            // conservait que le champ de mesure et perdait l'objet à un
            // aller-retour MusicXML (85 items du corpus).
            let index =
                pass2::get_segment(score, measure_index, SegmentType::EndBarLine, measure_end);
            score.measures[measure_index].segments[index]
                .set_element(track, SegmentItem::BarLine(BarLine::new(kind)));
        }
    }

    if let Some(repeat) = element.children_named("repeat").next() {
        let times = repeat
            .attribute("times")
            .and_then(|times| times.parse::<u8>().ok())
            .filter(|times| *times > 0)
            .unwrap_or(2);
        match repeat.attribute("direction") {
            Some("forward") => {
                score.measures[measure_index].start_bar_line = BarLineType::StartRepeat;
            }
            Some("backward") => {
                score.measures[measure_index].end_bar_line = BarLineType::EndRepeat;
                score.measures[measure_index].repeat_count = times;
            }
            _ => {}
        }
    }

    if let Some(ending) = element.child("ending") {
        read_ending(score, measure_index, ending, location, track);
    }

    for fermata in element.children_named("fermata") {
        let mut value = Fermata::new(fermata_type(fermata));
        value.placement = if fermata.attribute("type") == Some("inverted") {
            Placement::Below
        } else {
            Placement::Above
        };
        value.visible = fermata.attribute("print-object") != Some("no");
        let index = pass2::get_segment(
            score,
            measure_index,
            scoretk_core::model::SegmentType::ChordRest,
            measure_end,
        );
        score.measures[measure_index].segments[index]
            .add_annotation(track, Annotation::Fermata(value));
    }
}

/// Type de barre MusicXML.
fn barline_type(style: &str) -> BarLineType {
    match style {
        "light-light" => BarLineType::Double,
        "light-heavy" => BarLineType::End,
        "heavy-light" => BarLineType::StartRepeat,
        "heavy-heavy" => BarLineType::End,
        "dashed" => BarLineType::Broken,
        "dotted" => BarLineType::Dotted,
        "tick" => BarLineType::Tick,
        "short" => BarLineType::Short,
        _ => BarLineType::Normal,
    }
}

/// Type de point d'orgue.
fn fermata_type(element: &Element) -> FermataType {
    match element.text_trimmed() {
        "angled" => FermataType::Short,
        "square" => FermataType::Long,
        "double-angled" => FermataType::VeryShort,
        "double-square" => FermataType::VeryLong,
        "double-dot" => FermataType::LongHenze,
        "half-curve" => FermataType::ShortHenze,
        "curlew" => FermataType::Curlew,
        _ => FermataType::Normal,
    }
}

/// Lit un `<ending>` (`doEnding`).
fn read_ending(
    score: &mut Score,
    measure_index: usize,
    element: &Element,
    _location: &str,
    track: Track,
) {
    let kind = element.attribute("type").unwrap_or("");
    let endings = element
        .attribute("number")
        .unwrap_or("")
        .split([',', ' '])
        .filter_map(|number| number.trim().parse::<u8>().ok())
        .collect::<Vec<_>>();
    let text = element.text_trimmed();
    match kind {
        "start" => {
            let tick = score.measures[measure_index].start_tick;
            let anchor = SpannerAnchor::new(tick, track);
            let mut spanner = Spanner::new(SpannerType::Volta, anchor, anchor);
            spanner.text = Some(text.to_owned());
            spanner.volta = Some(VoltaInfo {
                endings,
                closed: false,
            });
            score.spanners.push(spanner);
        }
        "stop" | "discontinue" => {
            let tick = score.measures[measure_index].end_tick();
            let mut target = None;
            for (index, spanner) in score.spanners.iter().enumerate().rev() {
                if matches!(spanner.kind, SpannerType::Volta)
                    && spanner.end.tick == spanner.start.tick
                {
                    target = Some(index);
                    break;
                }
            }
            if let Some(index) = target {
                score.spanners[index].end = SpannerAnchor::new(tick, track);
                if let Some(info) = score.spanners[index].volta.as_mut() {
                    info.closed = kind == "stop";
                    if !endings.is_empty() {
                        info.endings = endings;
                    }
                }
            }
        }
        _ => {}
    }
}
