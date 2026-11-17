// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/measurewrite.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026 : écriture des mesures et des voix.
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

//! Mesures et voix.
//!
//! Une mesure s'écrit une fois par portée (`StaffWrite::writeStaff`). Dans
//! chaque mesure, les voix sont écrites dans l'ordre (`<voice>` est
//! positionnel pour le lecteur) et les éléments d'un même tick suivent l'ordre
//! des segments de MuseScore : éléments de système, annotations, accords.

use std::collections::BTreeMap;

use mscz2xml::model::{
    BarLine, BarLineType, ChordRest, Fraction, RepeatMark, SegmentAnnotation, SegmentItem,
    SegmentType, StemDirection, Tick, Track, TupletId, VOICES,
};

use crate::text::fraction;
use crate::write::connectorinfowriter::write_spanner_half;
use crate::write::context::{Half, WriteContext};
use crate::write::twrite::{
    placement_name, write_annotation, write_bar_line, write_chord_rest, write_item, write_tuplet,
};
use crate::xmlwriter::XmlWriter;

/// Écrit une mesure d'une portée, suivie du marqueur de multi-repos s'il y a
/// lieu (`StaffWrite::writeMeasure`).
pub(crate) fn write_measure(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    measure_index: usize,
    staff_index: usize,
) {
    let measure = &ctx.score.measures[measure_index];

    // Mesure répétée : son contenu est recopié par le lecteur à partir de la
    // mesure source (`MeasureRepeat`). On n'écrit donc que la marque.
    if let Some(target) = ctx.repeat_target(measure_index) {
        if target.staff == staff_index {
            xml.start_element("Measure", &[]);
            xml.start_element("voice", &[]);
            xml.start_element("MeasureRepeat", &[]);
            xml.tag("subtype", &target.count.to_string());
            xml.end_element();
            xml.end_element();
            xml.end_element();
            return;
        }
    }

    let len = fraction(measure.len);
    let attrs: Vec<(&str, &str)> = if measure.is_irregular() {
        vec![("len", len.as_str())]
    } else {
        Vec::new()
    };
    xml.start_element("Measure", &attrs);

    // Repères et sauts : ils sont rattachés à une piste de portée.
    for mark in &measure.repeat_marks {
        let track = match mark {
            RepeatMark::Marker(marker) => marker.track,
            RepeatMark::Jump(jump) => jump.track,
        };
        if track.staff().index() != staff_index {
            continue;
        }
        write_repeat_mark(xml, mark);
    }

    // Éléments de système, écrits pour la première portée seulement.
    if staff_index == 0 {
        if measure.start_bar_line.is_repeat_start() {
            xml.tag_empty_plain("startRepeat");
        }
        if measure.end_bar_line.is_repeat_end() {
            xml.tag("endRepeat", &measure.repeat_count.to_string());
        }
        // Le premier `<LayoutBreak>` est celui que `readStaff` consulte pour le
        // saut de section ; `readMeasure` parcourt tous les enfants pour le
        // saut de ligne. Le saut de section qui précède *cette* mesure est
        // porté par la précédente.
        let section_break = ctx
            .score
            .measures
            .get(measure_index + 1)
            .is_some_and(|next| next.section_break_before);
        if section_break {
            xml.start_element("LayoutBreak", &[]);
            xml.tag("subtype", "section");
            xml.end_element();
        }
        if measure.line_break {
            xml.start_element("LayoutBreak", &[]);
            xml.tag("subtype", "line");
            xml.end_element();
        }
    }

    write_voices(xml, ctx, measure_index, staff_index);
    xml.end_element();

    // Marqueur de multi-repos : la mesure qui ouvre le groupe est suivie d'une
    // mesure de mise en page qui porte `<multiMeasureRest>`.
    if let Some(count) = measure.multi_measure_rest {
        let total = measure.len * Fraction::from_integer(i64::from(count));
        let len = fraction(total);
        xml.start_element("Measure", &[("len", len.as_str())]);
        xml.tag("multiMeasureRest", &count.to_string());
        xml.end_element();
    }
}

/// Écrit un marqueur ou un saut attaché à la mesure.
fn write_repeat_mark(xml: &mut XmlWriter, mark: &RepeatMark) {
    match mark {
        RepeatMark::Marker(marker) => {
            xml.start_element("Marker", &[]);
            let label = if marker.label.is_empty() {
                crate::names::marker_label(marker.kind).to_owned()
            } else {
                marker.label.clone()
            };
            if !label.is_empty() {
                xml.tag("label", &label);
            }
            if !marker.text.is_empty() {
                xml.tag("text", &marker.text);
            }
            xml.tag("placement", placement_name(marker.placement));
            xml.end_element();
        }
        RepeatMark::Jump(jump) => {
            xml.start_element("Jump", &[]);
            if !jump.text.is_empty() {
                xml.tag("text", &jump.text);
            }
            if !jump.jump_to.is_empty() {
                xml.tag("jumpTo", &jump.jump_to);
            }
            if !jump.play_until.is_empty() {
                xml.tag("playUntil", &jump.play_until);
            }
            if !jump.continue_at.is_empty() {
                xml.tag("continueAt", &jump.continue_at);
            }
            xml.tag("placement", placement_name(jump.placement));
            xml.end_element();
        }
    }
}

/// Groupe d'événements d'une voix à un tick donné.
#[derive(Default)]
struct Group<'a> {
    halves: Vec<&'a Half<'a>>,
    system: Vec<(SegmentType, &'a SegmentItem)>,
    annotations: Vec<&'a SegmentAnnotation>,
    chords: Vec<&'a ChordRest>,
}

/// Vrai si la voix porte au moins un événement dans la mesure.
fn voice_used(ctx: &WriteContext<'_>, measure_index: usize, track: Track) -> bool {
    let measure = &ctx.score.measures[measure_index];
    let used = measure.segments.iter().any(|segment| {
        segment.element(track).is_some()
            || segment.annotations.iter().any(|entry| entry.track == track)
    });
    used || !ctx.halves_for(measure_index, track).is_empty()
}

/// Écrit les voix 0 à la dernière voix utilisée.
fn write_voices(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    measure_index: usize,
    staff_index: usize,
) {
    let staff = mscz2xml::model::StaffId::from_index(staff_index);
    let mut last = None;
    for voice in 0..VOICES as u8 {
        let track = Track::new(staff, voice);
        if voice_used(ctx, measure_index, track) {
            last = Some(voice);
        }
    }
    let Some(last) = last else {
        return;
    };
    for voice in 0..=last {
        let track = Track::new(staff, voice);
        write_voice(xml, ctx, measure_index, staff_index, voice, track);
    }
}

/// Écrit une voix d'une mesure.
fn write_voice(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    measure_index: usize,
    staff_index: usize,
    voice: u8,
    track: Track,
) {
    let measure = &ctx.score.measures[measure_index];
    let mut groups: BTreeMap<Tick, Group<'_>> = BTreeMap::new();

    for segment in &measure.segments {
        let group = groups.entry(segment.tick).or_default();
        if let Some(item) = segment.element(track) {
            match item {
                SegmentItem::ChordRest(chord_rest) => group.chords.push(chord_rest),
                _ => group.system.push((segment.kind, item)),
            }
        }
        for entry in &segment.annotations {
            if entry.track == track {
                group.annotations.push(entry);
            }
        }
    }
    for half in ctx.halves_for(measure_index, track) {
        let tick = if half.start {
            half.spanner.start.tick
        } else {
            half.spanner.end.tick
        };
        groups.entry(tick).or_default().halves.push(half);
    }

    xml.start_element("voice", &[]);
    let mut cursor = measure.start_tick;
    let mut open: Vec<TupletId> = Vec::new();

    for (tick, group) in groups {
        write_position(xml, tick, &mut cursor);
        for half in group.halves {
            write_spanner_half(xml, half);
        }
        for (kind, item) in group.system {
            write_system_item(xml, kind, item);
        }
        for annotation in group.annotations {
            write_annotation(xml, &annotation.annotation);
        }
        for chord_rest in group.chords {
            sync_tuplets(xml, ctx, chord_rest.tuplet, &mut open);
            if let Some(direction) = chord_rest.beam_direction {
                xml.start_element("Beam", &[]);
                xml.tag("StemDirection", stem_name(direction));
                xml.end_element();
            }
            write_chord_rest(xml, ctx, chord_rest, tick, track);
            cursor = tick + advance(ctx, measure_index, staff_index, chord_rest);
        }
    }
    // Ferme les n-olets restés ouverts.
    for _ in 0..open.len() {
        xml.tag_empty_plain("endTuplet");
    }

    // Barre de fin reconstruite à partir de `Measure::end_bar_line` lorsque le
    // fichier d'origine n'en portait pas (modèle construit, ou lecture 3.x).
    if voice == 0 {
        if let Some(bar_line) = synthetic_end_bar_line(ctx, measure_index, staff_index) {
            write_position(xml, measure.end_tick(), &mut cursor);
            write_bar_line(xml, &bar_line);
        }
    }
    xml.end_element();
}

/// Nom MuseScore d'une direction de hampe.
fn stem_name(stem: StemDirection) -> &'static str {
    match stem {
        StemDirection::Up => "up",
        StemDirection::Down => "down",
        StemDirection::Auto => "auto",
    }
}

/// Écrit la position d'un groupe (`<location><fractions>`).
fn write_position(xml: &mut XmlWriter, tick: Tick, cursor: &mut Tick) {
    if tick == *cursor {
        return;
    }
    let delta = tick - *cursor;
    xml.start_element("location", &[]);
    xml.tag("fractions", &fraction(delta));
    xml.end_element();
    *cursor = tick;
}

/// Écrit un élément de système (clef, armure, mesure, barre, respiration).
fn write_system_item(xml: &mut XmlWriter, kind: SegmentType, item: &SegmentItem) {
    if let SegmentItem::Clef(clef) = item {
        let header = matches!(
            kind,
            SegmentType::HeaderClef
                | SegmentType::ClefStartRepeatAnnounce
                | SegmentType::ClefRepeatAnnounce
        );
        crate::write::twrite::write_clef(xml, clef, header);
        return;
    }
    write_item(xml, item);
}

/// Ouvre et ferme les n-olets pour que la pile du lecteur soit celle du modèle.
fn sync_tuplets(
    xml: &mut XmlWriter,
    ctx: &WriteContext<'_>,
    tuplet: Option<TupletId>,
    open: &mut Vec<TupletId>,
) {
    let chain = tuplet_chain(ctx, tuplet);
    // Ferme les n-olets qui ne contiennent plus l'accord.
    while let Some(last) = open.last() {
        if chain.contains(last) {
            break;
        }
        open.pop();
        xml.tag_empty_plain("endTuplet");
    }
    // Ouvre ceux qui manquent, du plus externe au plus interne.
    for id in chain.iter().skip(open.len()) {
        if let Some(value) = ctx.score.tuplets.get(id.index()) {
            write_tuplet(xml, value);
        }
        open.push(*id);
    }
}

/// Chaîne des n-olets d'un accord, du plus externe au plus interne.
fn tuplet_chain(ctx: &WriteContext<'_>, tuplet: Option<TupletId>) -> Vec<TupletId> {
    let mut chain = Vec::new();
    let mut current = tuplet;
    while let Some(id) = current {
        if chain.contains(&id) || chain.len() > 32 {
            break;
        }
        chain.push(id);
        current = ctx
            .score
            .tuplets
            .get(id.index())
            .and_then(|value| value.parent);
    }
    chain.reverse();
    chain
}

/// Durée réelle d'un accord ou d'un silence, telle que le lecteur l'avancera
/// (`DurationElement::actualTicks`).
fn advance(
    ctx: &WriteContext<'_>,
    measure_index: usize,
    staff_index: usize,
    chord_rest: &ChordRest,
) -> Fraction {
    let measure = &ctx.score.measures[measure_index];
    let stretch = ctx.stretch_at(measure_index, staff_index);
    let mut ticks = match chord_rest.ticks {
        Some(ticks) => ticks,
        None => {
            if chord_rest.duration.is_measure() {
                let nominal = measure.nominal_len;
                if stretch.is_positive() {
                    nominal / stretch
                } else {
                    nominal
                }
            } else {
                chord_rest.duration.ticks()
            }
        }
    };
    let mut current = chord_rest.tuplet;
    let mut depth = 0;
    while let Some(id) = current {
        let Some(value) = ctx.score.tuplets.get(id.index()) else {
            break;
        };
        if value.ratio.is_positive() {
            ticks /= value.ratio;
        }
        current = value.parent;
        depth += 1;
        if depth > 32 {
            break;
        }
    }
    if stretch.is_positive() {
        ticks /= stretch;
    }
    ticks
}

/// Barre de fin à reconstruire quand le modèle n'en porte aucune.
fn synthetic_end_bar_line(
    ctx: &WriteContext<'_>,
    measure_index: usize,
    staff_index: usize,
) -> Option<BarLine> {
    let measure = &ctx.score.measures[measure_index];
    if measure.end_bar_line == BarLineType::Normal || measure.end_bar_line.is_repeat_end() {
        return None;
    }
    // Une barre de fin lue dans le fichier a déjà son segment `EndBarLine`.
    let has_item = measure.segments.iter().any(|segment| {
        segment.kind == SegmentType::EndBarLine
            && segment.tick == measure.end_tick()
            && segment.iter_elements().any(|(track, item)| {
                track.staff().index() == staff_index && matches!(item, SegmentItem::BarLine(_))
            })
    });
    if has_item {
        return None;
    }
    // Une seule barre suffit : le lecteur met à jour `Measure::end_bar_line`
    // pour chaque portée, mais la valeur est globale.
    let target = ctx.repeat_target(measure_index);
    let first_with_content = (0..ctx.score.staves.len())
        .find(|staff| target.is_none_or(|value| value.staff != *staff))
        .unwrap_or(0);
    if staff_index != first_with_content {
        return None;
    }
    Some(BarLine::new(measure.end_bar_line))
}
