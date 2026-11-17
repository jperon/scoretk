// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` de
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

//! Chiffrages figurés à l'export (`figuredBass`, `writeMusicXml`).
//!
//! Un chiffrage peut durer plus longtemps que la note qui le porte : son trait
//! de prolongation est alors réparti sur les accords suivants (`FigBassMap`
//! dans le C++). Le suivi est propre à chaque partie.

use std::collections::BTreeMap;

use crate::model::{
    Annotation, ChordRest, FiguredBass, FiguredBassContLine, FiguredBassItem, FiguredBassModifier,
    Measure, Segment, StaffId, Tick,
};

use super::exportmusicxml::{Context, Out};

/// Chiffrage en cours de prolongation : tick de son segment et chiffrage
/// (`FigBassMap`).
#[derive(Default)]
pub(crate) struct Pending(BTreeMap<StaffId, (Tick, FiguredBass)>);

/// Écrit les chiffrages figurés du segment (`figuredBass`).
pub(crate) fn write(
    context: &Context<'_>,
    out: &mut Out,
    measure: &Measure,
    segment: &Segment,
    chord_rest: &ChordRest,
    staff_id: StaffId,
    pending: &mut Pending,
) {
    if !segment.kind.is_chord_rest_type() {
        return;
    }
    let actual = context.actual_ticks(chord_rest, measure, staff_id);
    let chord_end = segment.tick + actual;
    for entry in &segment.annotations {
        if entry.track.staff() != staff_id {
            continue;
        }
        let Annotation::FiguredBass(bass) = &entry.annotation else {
            continue;
        };
        // `fb->items().empty()` : un chiffrage vide n'est pas exporté.
        if bass.items.is_empty() {
            continue;
        }
        let bass_end = segment.tick + bass.ticks;
        if bass.ticks > actual {
            pending.0.insert(staff_id, (segment.tick, bass.clone()));
        } else {
            pending.0.remove(&staff_id);
        }
        let write_duration = bass.ticks < actual;
        write_musicxml(out, bass, true, chord_end, bass_end, write_duration);
        // Chiffres changeant sous une même note : chacun occupe son segment,
        // jusqu'au prochain élément de la piste.
        if let Some(position) = measure
            .segments
            .iter()
            .position(|candidate| std::ptr::eq(candidate, segment))
        {
            for next in measure.segments.iter().skip(position + 1) {
                if next.element(entry.track).is_some() {
                    break;
                }
                for next_entry in &next.annotations {
                    if next_entry.track != entry.track {
                        continue;
                    }
                    if let Annotation::FiguredBass(next_bass) = &next_entry.annotation {
                        write_musicxml(out, next_bass, true, Tick::ZERO, Tick::ZERO, true);
                    }
                }
            }
        }
        return;
    }

    // Un chiffrage précédent se prolonge sous cet accord.
    let Some((bass_tick, bass)) = pending.0.get(&staff_id) else {
        return;
    };
    let bass_end = *bass_tick + bass.ticks;
    let write_duration = bass.ticks < actual;
    if segment.tick < bass_end {
        write_musicxml(out, bass, false, chord_end, bass_end, write_duration);
    }
    if bass_end <= chord_end {
        pending.0.remove(&staff_id);
    }
}

/// Écrit un `<figured-bass>` (`writeMusicXml(const FiguredBass*)`).
fn write_musicxml(
    out: &mut Out,
    bass: &FiguredBass,
    is_original: bool,
    chord_end: Tick,
    bass_end: Tick,
    write_duration: bool,
) {
    out.attr(false);
    let mut attributes: Vec<(&str, &str)> = Vec::new();
    if bass.parentheses {
        attributes.push(("parentheses", "yes"));
    }
    if bass.place_above {
        attributes.push(("placement", "above"));
    }
    if !bass.visible {
        attributes.push(("print-object", "no"));
    }
    out.xml.start_element("figured-bass", &attributes);
    for item in &bass.items {
        write_item(out, item, is_original, chord_end, bass_end);
    }
    if write_duration {
        out.xml.tag(
            "duration",
            &out.duration_in_divisions(bass.ticks).to_string(),
        );
    }
    out.xml.end_element();
}

/// Écrit un `<figure>` (`writeMusicXml(const FiguredBassItem*)`).
fn write_item(
    out: &mut Out,
    item: &FiguredBassItem,
    is_original: bool,
    chord_end: Tick,
    bass_end: Tick,
) {
    out.xml.start_element("figure", &[]);
    if is_original {
        write_modifier(out, "prefix", item.prefix);
        if let Some(digit) = item.digit {
            out.xml.tag("figure-number", &digit.to_string());
        }
        write_modifier(out, "suffix", item.suffix);
        let has_figure = item
            .prefix
            .is_some_and(|modifier| modifier.musicxml_name().is_some())
            || item.digit.is_some()
            || item
                .suffix
                .is_some_and(|modifier| modifier.musicxml_name().is_some());
        if bass_end <= chord_end {
            match item.cont_line {
                FiguredBassContLine::Simple => {
                    out.xml.tag_empty("extend", &[("type", "stop")]);
                }
                FiguredBassContLine::Extended => {
                    let kind = if has_figure { "start" } else { "continue" };
                    out.xml.tag_empty("extend", &[("type", kind)]);
                }
                FiguredBassContLine::None => {}
            }
        } else {
            out.xml.tag_empty("extend", &[("type", "start")]);
        }
    } else if chord_end < bass_end {
        out.xml.tag_empty("extend", &[("type", "continue")]);
    } else {
        out.xml.tag_empty("extend", &[("type", "stop")]);
    }
    out.xml.end_element();
}

/// Écrit un `<prefix>`/`<suffix>` (`Modifier2MusicXml`).
fn write_modifier(out: &mut Out, name: &str, modifier: Option<FiguredBassModifier>) {
    if let Some(modifier) = modifier.and_then(FiguredBassModifier::musicxml_name) {
        out.xml.tag(name, modifier);
    }
}
