// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiexporter.cpp` de MuseScore Studio
// 4.7.4, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026. Les noms suivent le C++ (`writeScore`, `writeMeasure`,
// `writeStaff`, `writeLayer`, `writeChord`, `writeNote`, `writeRest`,
// `writeBeamAndTuplet`, …).
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

//! Wooden, deterministic MEI writer: one `<staff>`/`<layer>` per voice and
//! control events after the staves, as MuseScore 4.7.4 writes them.

use std::collections::BTreeMap;

use scoretk_core::model::{
    Annotation, BeamMode, ChordRest, ChordRestKind, DurationType, Fraction, Key, Score,
    SegmentItem, SpannerNotation, SpannerType, TimeSigType, Track, VOICES,
};
use scoretk_mei_common::{convert, ids};
use scoretk_xml_common::XmlWriter;

use crate::{header, MeiOptions};

/// `MeiExporter::write` (`meiexporter.cpp:100`).
pub(crate) fn write(
    score: &Score,
    options: &MeiOptions,
) -> Result<String, scoretk_mei_common::Error> {
    let mut xml = XmlWriter::new();
    header::mei_start(&mut xml, &options.encoding_date);
    header::write_header(&mut xml, score, &options.encoding_date);
    xml.start_element("music", &[]);
    xml.start_element("body", &[]);
    xml.start_element("mdiv", &[]);
    xml.start_element("score", &[]);
    header::write_score_def(&mut xml, score, options);
    xml.start_element("section", &[("xml:id", "s1")]);
    let mut measure_n = 0;
    let mut signatures = measure_signatures(score.measures.first());
    for (index, measure) in score.measures.iter().enumerate() {
        let current = measure_signatures(Some(measure));
        write_score_def_change(&mut xml, &signatures, &current);
        signatures = current;
        if options.full_support {
            write_transposition_change(&mut xml, score, index);
        }
        write_ending_start(&mut xml, score, index);
        write_measure(&mut xml, score, measure, index, &mut measure_n, options);
        write_ending_end(&mut xml, score, index);
    }
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    Ok(xml.finish())
}

/// `(meter, key)` in force at the beginning of a measure.
type Signatures = (Option<(i32, i32, TimeSigType)>, Option<Key>);

fn measure_signatures(measure: Option<&scoretk_core::model::Measure>) -> Signatures {
    let Some(measure) = measure else {
        return (None, None);
    };
    let mut time = None;
    let mut key = None;
    for segment in &measure.segments {
        for (_, item) in segment.iter_elements() {
            match item {
                SegmentItem::TimeSig(sig) if time.is_none() => {
                    time = Some((sig.numerator, sig.denominator, sig.kind));
                }
                SegmentItem::KeySig(sig) if key.is_none() => {
                    key = Some(sig.concert_key);
                }
                _ => {}
            }
        }
    }
    (time, key)
}

/// `MeiExporter::writeScoreDefChange` (`meiexporter.cpp:520`).
fn write_score_def_change(xml: &mut XmlWriter, previous: &Signatures, current: &Signatures) {
    if current == previous || (current.0.is_none() && current.1.is_none()) {
        return;
    }
    let mut attrs = Vec::new();
    if let Some((numerator, denominator, kind)) = current.0 {
        if previous.0 != current.0 {
            let time = scoretk_core::model::TimeSig {
                numerator,
                denominator,
                kind,
                ..scoretk_core::model::TimeSig::new(1, 1)
            };
            let (count, unit, sym) = convert::meter_to_mei(&time);
            if let Some(count) = count {
                attrs.push(("meter.count".to_owned(), count));
            }
            if let Some(unit) = unit {
                attrs.push(("meter.unit".to_owned(), unit.to_string()));
            }
            if let Some(sym) = sym {
                attrs.push(("meter.sym".to_owned(), sym.to_owned()));
            }
        }
    }
    if let Some(key) = current.1 {
        if previous.1 != current.1 {
            attrs.push(("keysig".to_owned(), convert::key_to_mei(key)));
        }
    }
    if attrs.is_empty() {
        return;
    }
    let refs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("scoreDef", &refs);
    xml.end_element();
}

/// Transposition effective on `staff_idx` at `tick`: the initial
/// `part.instrument.transpose`, overridden by each `InstrumentChange`
/// annotation (`InstrumentChange::instrument.transpose`) up to that tick.
fn effective_transpose(score: &Score, staff_idx: usize, tick: Fraction) -> i32 {
    let mut value = score
        .staves
        .get(staff_idx)
        .and_then(|staff| score.parts.get(staff.part.index()))
        .and_then(|part| part.instrument.transpose)
        .map_or(0, |transpose| transpose.chromatic);
    for measure in &score.measures {
        if measure.start_tick > tick {
            break;
        }
        for segment in &measure.segments {
            if segment.tick > tick {
                continue;
            }
            for annotation in &segment.annotations {
                if annotation.track.staff().raw() as usize != staff_idx {
                    continue;
                }
                if let Annotation::InstrumentChange(change) = &annotation.annotation {
                    if let Some(instrument) = &change.instrument {
                        value = instrument
                            .transpose
                            .map_or(0, |transpose| transpose.chromatic);
                    }
                }
            }
        }
    }
    value
}

/// `<scoreDef>` written before a measure whose effective transposition
/// changes, so the reader applies the new written/concert pitch relationship
/// (`Convert::staffToMEI`, meiconverter.cpp:3099).
fn write_transposition_change(xml: &mut XmlWriter, score: &Score, measure_index: usize) {
    let Some(measure) = score.measures.get(measure_index) else {
        return;
    };
    let tick = measure.start_tick;
    for (staff_idx, _) in score.staves.iter().enumerate() {
        let current = effective_transpose(score, staff_idx, tick);
        let previous = if measure_index == 0 {
            current
        } else {
            effective_transpose(
                score,
                staff_idx,
                score.measures[measure_index - 1].start_tick,
            )
        };
        if current == previous {
            continue;
        }
        let diatonic = score
            .staves
            .get(staff_idx)
            .and_then(|staff| score.parts.get(staff.part.index()))
            .and_then(|part| part.instrument.transpose)
            .map_or(0, |transpose| transpose.diatonic);
        // Le nombre diatonique n'est pas modifié par un changement d'armure
        // d'instrument dans les fichiers de référence ; conserver celui de la
        // partie suffit pour que la relecture reconstruise le concert pitch.
        xml.start_element("scoreDef", &[]);
        let n = (staff_idx + 1).to_string();
        let diatonic = diatonic.to_string();
        let chromatic = current.to_string();
        xml.tag_empty(
            "staffDef",
            &[
                ("n", &n),
                ("trans.diat", &diatonic),
                ("trans.semi", &chromatic),
            ],
        );
        xml.end_element();
    }
}

/// `xml:id` stable de chaque événement (note ou silence) d'une mesure. Les
/// silences portent aussi un id quand les porteurs additionnels sont actifs :
/// les hairpins et les text-lines s'ancrent souvent sur un silence, pas sur
/// une note (`@tstamp` seul perd la précision du tick).
fn event_ids(score: &Score, full_support: bool) -> BTreeMap<(Fraction, u32), String> {
    let mut map = BTreeMap::new();
    for (measure_index, measure) in score.measures.iter().enumerate() {
        for staff_idx in 0..score.staves.len() {
            let staff_id = scoretk_core::model::StaffId::from_index(staff_idx);
            for voice in 0..VOICES as u8 {
                let track = Track::new(staff_id, voice);
                let events: Vec<_> = measure
                    .segments
                    .iter()
                    .filter_map(|segment| {
                        segment
                            .element(track)
                            .and_then(SegmentItem::as_chord_rest)
                            .map(|cr| (segment.tick, cr))
                    })
                    .collect();
                for (position, (tick, cr)) in events.iter().enumerate() {
                    let id = match &cr.kind {
                        ChordRestKind::Chord(chord) => {
                            chord.notes.first().map(|note| format!("n{}", note.index()))
                        }
                        ChordRestKind::Rest(rest) if rest.full_measure => {
                            full_support.then(|| format!("mr{measure_index}"))
                        }
                        ChordRestKind::Rest(_) => full_support
                            .then(|| format!("r{measure_index}s{}i{position}", track.raw())),
                    };
                    if let Some(id) = id {
                        map.entry((*tick, track.raw())).or_insert(id);
                    }
                }
            }
        }
    }
    map
}

/// Measure containing `tick`, used to place `<ending>` around its measures.
fn measure_at(score: &Score, tick: Fraction) -> usize {
    score
        .measures
        .iter()
        .position(|measure| tick >= measure.start_tick && tick < measure.end_tick())
        .unwrap_or(score.measures.len().saturating_sub(1))
}

/// Measure whose *end* is `tick` (a volta ends on its last measure's end tick).
fn measure_at_end(score: &Score, tick: Fraction) -> usize {
    score
        .measures
        .iter()
        .rposition(|measure| measure.end_tick() == tick)
        .unwrap_or_else(|| measure_at(score, tick))
}

/// `MeiExporter::writeEnding` (`meiexporter.cpp:796`).
fn write_ending_start(xml: &mut XmlWriter, score: &Score, index: usize) {
    for spanner in &score.spanners {
        let SpannerType::Volta = &spanner.kind else {
            continue;
        };
        if measure_at(score, spanner.start.tick) != index {
            continue;
        }
        let Some(info) = &spanner.volta else {
            continue;
        };
        let types = info
            .endings
            .iter()
            .map(|number| format!("{}{number}", ids::ENDING_REPEAT_TYPE))
            .collect::<Vec<_>>()
            .join(" ");
        let label = info
            .endings
            .iter()
            .map(|number| format!("{number}."))
            .collect::<Vec<_>>()
            .join(" ");
        let mut attrs = vec![("label".to_owned(), label)];
        if !types.is_empty() {
            attrs.push(("type".to_owned(), types));
        }
        if !info.closed {
            attrs.push(("lendsym".to_owned(), "none".to_owned()));
        }
        let refs: Vec<(&str, &str)> = attrs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        xml.start_element("ending", &refs);
    }
}

/// `MeiExporter::writeEndingEnd` (`meiexporter.cpp:814`).
fn write_ending_end(xml: &mut XmlWriter, score: &Score, index: usize) {
    for spanner in &score.spanners {
        if spanner.kind == SpannerType::Volta && measure_at_end(score, spanner.end.tick) == index {
            xml.end_element();
        }
    }
}

/// `MeiExporter::writeMeasure` (`meiexporter.cpp:834`).
fn write_measure(
    xml: &mut XmlWriter,
    score: &Score,
    measure: &scoretk_core::model::Measure,
    index: usize,
    measure_n: &mut i32,
    options: &MeiOptions,
) {
    // `MeiExporter::writeScore` (`meiexporter.cpp:296-307`) writes the break
    // *before* the measure that follows it; the reader attaches it to the
    // previous measure (`meiimporter.cpp:1502-1543`).
    if index > 0 {
        let previous = &score.measures[index - 1];
        if options.layout && previous.section_break_before {
            xml.tag_empty("pb", &[("type", ids::BREAK_TYPE)]);
        }
        if options.layout && previous.line_break {
            xml.tag_empty("sb", &[("type", ids::BREAK_TYPE)]);
        }
    }
    let mut attrs = convert::measure_to_mei(measure, measure_n)
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect::<Vec<_>>();
    attrs.insert(0, ("xml:id".to_owned(), format!("m{index}")));
    let refs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("measure", &refs);
    for (staff_idx, _) in score.staves.iter().enumerate() {
        let staff_id = scoretk_core::model::StaffId::from_index(staff_idx);
        let has_content = (0..VOICES as u8).any(|voice| {
            let track = Track::new(staff_id, voice);
            measure
                .segments
                .iter()
                .any(|segment| segment.element(track).is_some())
        });
        if !has_content {
            continue;
        }
        xml.start_element(
            "staff",
            &[
                ("xml:id", &format!("m{}s{}", index + 1, staff_idx + 1)),
                ("n", &(staff_idx + 1).to_string()),
            ],
        );
        for voice in 0..VOICES as u8 {
            let track = Track::new(staff_id, voice);
            write_layer(xml, score, measure, track, voice, index, options);
        }
        xml.end_element();
    }
    write_control_events(xml, score, measure, index, options);
    if options.full_support {
        for (tbox_index, text) in score.engraving.tbox_texts.iter().enumerate() {
            if text.measure != index {
                continue;
            }
            let size = text
                .font_size
                .map(|value| format!("{value:.5}"))
                .unwrap_or_else(|| "none".to_owned());
            let label = format!(
                "{}{}:{:?}:{}",
                ids::TBOX_FRAME_LABEL,
                text.measure,
                text.placement,
                size
            );
            let type_name = format!("{}{}", ids::TBOX_TYPE, tbox_index);
            let attrs = [
                ("staff", "1"),
                ("tstamp", "1.000"),
                ("type", type_name.as_str()),
                ("label", label.as_str()),
            ];
            xml.tag_with("dir", &attrs, text.text.plain());
        }
    }
    // `MeiExporter::writeMeasure`: jumps and markers are direct children of
    // the measure (`writeRepeatMark`, meiexporter.cpp:2057/2112).
    for mark in &measure.repeat_marks {
        let (type_name, func, text) = match mark {
            scoretk_core::model::RepeatMark::Marker(marker) => {
                let func = if marker.kind.is_stop_marker()
                    && marker.kind != scoretk_core::model::MarkerType::Fine
                {
                    "coda"
                } else {
                    match marker.kind {
                        scoretk_core::model::MarkerType::Segno
                        | scoretk_core::model::MarkerType::VarSegno => "segno",
                        scoretk_core::model::MarkerType::Fine => "fine",
                        _ => "coda",
                    }
                };
                (
                    format!(
                        "{}{}",
                        ids::MARKER_TYPE,
                        convert::marker_type_to_mei(marker.kind)
                    ),
                    func,
                    // MEI 5.1 `<repeatMark>` : le texte porte l'étiquette
                    // MuseScore (label), distinct du libellé affiché.
                    marker.label.clone(),
                )
            }
            scoretk_core::model::RepeatMark::Jump(jump) => {
                let func = if jump.kind.is_da_capo() {
                    "daCapo"
                } else {
                    "dalSegno"
                };
                (
                    format!("{}{}", ids::JUMP_TYPE, convert::jump_type_to_mei(jump.kind)),
                    func,
                    jump.jump_to.clone(),
                )
            }
        };
        xml.tag_with(
            "repeatMark",
            &[("type", &type_name), ("func", func), ("tstamp", "0")],
            &text,
        );
    }
    xml.end_element();
}

/// Écrit les accords d'agrément d'un accord dans un `<graceGrp>` MEI 5.1.
/// `@grace` et `@dur` sont standard ; `@type="mscore-grace-*"` porte la
/// variante exacte (les huit `GraceType` de MuseScore ne se réduisent pas aux
/// trois valeurs de `data.GRACE`). Porteur additionnel.
fn write_graces(
    xml: &mut XmlWriter,
    score: &Score,
    chord: &scoretk_core::model::Chord,
    transpose: i32,
) {
    for grace in &chord.grace_chords {
        let Some(grace_type) = grace.chord.grace else {
            continue;
        };
        let duration = convert::dur_to_mei(grace.duration.kind).to_owned();
        let dots = grace.duration.dots.to_string();
        let grace_name = format!(
            "{}{}",
            ids::GRACE_TYPE,
            convert::grace_type_name(grace_type)
        );
        let mut attrs = vec![
            (
                "grace".to_owned(),
                convert::grace_standard(grace_type).to_owned(),
            ),
            ("type".to_owned(), grace_name),
            ("dur".to_owned(), duration),
        ];
        if grace.duration.dots > 0 {
            attrs.push(("dots".to_owned(), dots));
        }
        let refs: Vec<(&str, &str)> = attrs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        xml.start_element("graceGrp", &refs);
        for id in &grace.chord.notes {
            let note = &score.notes[id.index()];
            let (pname, oct, accid) = convert::pitch_to_mei_written(note.pitch, transpose);
            let oct = oct.to_string();
            let mut note_attrs = vec![("pname", pname.as_str()), ("oct", oct.as_str())];
            if let Some(accid) = accid {
                note_attrs.push(("accid", accid));
            }
            xml.tag_empty("note", &note_attrs);
        }
        xml.end_element();
    }
}

/// `MeiExporter::writeLayer` (`meiexporter.cpp:964`).
fn write_layer(
    xml: &mut XmlWriter,
    score: &Score,
    measure: &scoretk_core::model::Measure,
    track: Track,
    voice: u8,
    measure_index: usize,
    options: &MeiOptions,
) {
    let events: Vec<(Fraction, &ChordRest)> = measure
        .segments
        .iter()
        .filter_map(|segment| {
            segment
                .element(track)
                .and_then(SegmentItem::as_chord_rest)
                .map(|cr| (segment.tick, cr))
        })
        .collect();
    if events.is_empty() {
        return;
    }
    let layer_id = format!(
        "m{}s{}l{}",
        measure_index + 1,
        track.staff().raw() + 1,
        voice + 1
    );
    let layer_n = (voice + 1).to_string();
    // `MeiExporter::writeNote` (meiexporter.cpp:1370) appelle
    // `Convert::pitchToMEI(note, accid, staff->part()->instrument()->transpose())`
    // : la hauteur écrite est la hauteur réelle moins la transposition.
    let initial_transpose = score
        .staves
        .get(track.staff().raw() as usize)
        .and_then(|staff| score.parts.get(staff.part.index()))
        .and_then(|part| part.instrument.transpose)
        .map_or(0, |value| value.chromatic);
    xml.start_element("layer", &[("xml:id", &layer_id), ("n", &layer_n)]);
    // MEI `<meterSig>` carries a time signature that changes inside a measure,
    // which a `<scoreDef>` at the measure boundary cannot express.
    let den = convert::tstamp_denominator(measure);
    for segment in &measure.segments {
        if segment.tick == measure.start_tick {
            continue;
        }
        if let Some(SegmentItem::TimeSig(time)) = segment.element(track) {
            let (count, unit, sym) = convert::meter_to_mei(time);
            let mut attrs = Vec::new();
            if let Some(count) = count {
                attrs.push(("count".to_owned(), count));
            }
            if let Some(unit) = unit {
                attrs.push(("unit".to_owned(), unit.to_string()));
            }
            if let Some(sym) = sym {
                attrs.push(("sym".to_owned(), sym.to_owned()));
            }
            attrs.push((
                "tstamp".to_owned(),
                format!(
                    "{:.3}",
                    convert::tstamp_from_fraction(segment.tick - measure.start_tick, den)
                ),
            ));
            let refs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            xml.tag_empty("meterSig", &refs);
        }
    }
    // `<clef>` inside a layer carries a clef change within the measure.
    for segment in &measure.segments {
        if segment.tick == measure.start_tick {
            continue;
        }
        if let Some(SegmentItem::Clef(clef)) = segment.element(track) {
            let (shape, line, dis, place) = convert::clef_to_mei(clef.concert);
            let line_owned = line.to_string();
            let mut attrs = vec![("shape", shape), ("line", &line_owned)];
            if let Some(dis) = dis {
                attrs.push(("dis", dis));
            }
            if let Some(place) = place {
                attrs.push(("dis.place", place));
            }
            let stamp = format!(
                "{:.3}",
                convert::tstamp_from_fraction(segment.tick - measure.start_tick, den)
            );
            attrs.push(("tstamp", &stamp));
            xml.tag_empty("clef", &attrs);
        }
    }
    let mut open_tuplets: Vec<scoretk_core::model::TupletId> = Vec::new();
    let mut open_beam = false;
    for (position, (tick, cr)) in events.iter().enumerate() {
        let transpose = if options.full_support {
            effective_transpose(score, track.staff().raw() as usize, *tick)
        } else {
            initial_transpose
        };
        if options.full_support {
            if let ChordRestKind::Chord(chord) = &cr.kind {
                write_graces(xml, score, chord, transpose);
            }
        }
        // MuseScore wraps consecutive beamed events in a `<beam>` container.
        if cr.engraving.beam_mode == BeamMode::Begin && !open_beam {
            xml.start_element("beam", &[]);
            open_beam = true;
        }
        let mut attributes = Vec::new();
        attributes.push((
            "dur".to_owned(),
            convert::dur_to_mei(cr.duration.kind).to_owned(),
        ));
        if cr.duration.dots > 0 {
            attributes.push(("dots".to_owned(), cr.duration.dots.to_string()));
        }
        // `att.gestural.log/@dur.ges` (MEI 5.1) carries MuseScore's
        // `actualTicks()` when it differs from the written `durationType`.
        if let Some(actual) = cr.ticks {
            if actual != cr.duration.ticks() {
                if let Some((kind, dots)) = convert::duration_from_ticks(actual) {
                    attributes.push(("dur.ges".to_owned(), convert::dur_to_mei(kind).to_owned()));
                    if dots > 0 {
                        attributes.push(("dots.ges".to_owned(), dots.to_string()));
                    }
                }
            }
        }
        // `MeiExporter::writeStemAtt` (`meiexporter.cpp:2287`): `@stem.dir`
        // is written on the chord/note event, not on a dedicated element.
        if let ChordRestKind::Chord(chord) = &cr.kind {
            if let Some(dir) = convert::stem_to_mei(chord.engraving.stem) {
                attributes.push(("stem.dir".to_owned(), dir.to_owned()));
            }
        }
        let beam = convert::beam_to_mei(cr.engraving.beam_mode, ids::BEAM_ELEMENT_TYPE);
        if !beam.is_empty() {
            attributes.push(("type".to_owned(), beam));
        }
        if let Some(breaksec) = convert::breaksec_to_mei(cr.engraving.beam_mode) {
            attributes.push(("breaksec".to_owned(), breaksec.to_string()));
        }
        // MuseScore 4.7.4 n'écrit qu'un n-olet (`writeTuplet`,
        // meiexporter.cpp:1544-1561 : « nested tuplet export not fully
        // supported »). MEI 5.1 autorise les `<tuplet>` imbriqués : écrire
        // la chaîne complète permet au lecteur de retrouver le modèle.
        let chain = tuplet_chain(score, cr.tuplet);
        let common = open_tuplets
            .iter()
            .zip(&chain)
            .take_while(|(left, right)| left == right)
            .count();
        for _ in common..open_tuplets.len() {
            xml.end_element();
        }
        open_tuplets.truncate(common);
        for id in &chain[common..] {
            let tuplet = &score.tuplets[id.index()];
            let place = match tuplet.placement {
                Some(scoretk_core::model::Placement::Below) => "below",
                _ => "above",
            };
            let mut attrs = vec![
                ("num".to_owned(), tuplet.actual_notes.to_string()),
                ("numbase".to_owned(), tuplet.normal_notes.to_string()),
            ];
            if tuplet.bracket {
                attrs.push(("bracket.place".to_owned(), place.to_owned()));
                attrs.push(("num.place".to_owned(), place.to_owned()));
            } else if tuplet.bracket_explicit {
                attrs.push(("bracket.visible".to_owned(), "false".to_owned()));
            }
            let refs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            xml.start_element("tuplet", &refs);
            open_tuplets.push(*id);
        }
        let stamp = convert::tstamp_from_fraction(
            *tick - measure.start_tick,
            convert::tstamp_denominator(measure),
        );
        attributes.push(("tstamp".to_owned(), format!("{stamp:.3}")));
        let refs: Vec<(&str, &str)> = attributes
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        // `MeiExporter::writeChord`: tremolos wrap the note/chord event.
        let tremolo = match &cr.kind {
            ChordRestKind::Chord(chord) => chord.tremolo,
            _ => None,
        };
        let two_chord = match &cr.kind {
            ChordRestKind::Chord(chord) => chord.tremolo_two_chord,
            _ => false,
        };
        let open_tremolo = tremolo.is_some() || two_chord;
        if open_tremolo {
            if two_chord {
                xml.start_element("fTrem", &[("unitdur", "32")]);
            } else {
                xml.start_element("bTrem", &[]);
            }
        }
        match &cr.kind {
            ChordRestKind::Rest(rest) if rest.full_measure => {
                xml.tag_empty("mRest", &[("xml:id", &format!("mr{measure_index}"))]);
            }
            ChordRestKind::Rest(rest) => {
                let rest_id = options
                    .full_support
                    .then(|| format!("r{measure_index}s{}i{position}", track.raw()));
                let mut rest_attrs = refs.clone();
                if !rest.visible {
                    rest_attrs.push(("visible", "false"));
                }
                if let Some(id) = &rest_id {
                    rest_attrs.push(("xml:id", id.as_str()));
                }
                xml.start_element("rest", &rest_attrs);
                write_verses(xml, cr);
                xml.end_element();
            }
            ChordRestKind::Chord(chord) => {
                if chord.notes.len() == 1 {
                    write_note(
                        xml,
                        score,
                        &chord.notes[0],
                        &refs,
                        Some(chord),
                        cr.duration.kind,
                        NotePitchContext {
                            transpose,
                            full_support: options.full_support,
                        },
                    );
                    // `MeiExporter::writeVerse` (meiexporter.cpp:1594) attache
                    // `<verse>` à l'élément `<note>` ouvert : MEI 5.1 définit
                    // `verse` comme enfant de `note`/`chord`, pas comme frère.
                    write_verses(xml, cr);
                    xml.end_element();
                } else {
                    xml.start_element("chord", &refs);
                    for id in &chord.notes {
                        write_note(
                            xml,
                            score,
                            id,
                            &[],
                            None,
                            cr.duration.kind,
                            NotePitchContext {
                                transpose,
                                full_support: options.full_support,
                            },
                        );
                        xml.end_element();
                    }
                    write_artics(xml, chord, options.full_support);
                    write_verses(xml, cr);
                    xml.end_element();
                }
            }
        }
        if open_tremolo {
            xml.end_element();
        }
        if open_beam
            && matches!(
                cr.engraving.beam_mode,
                BeamMode::End | BeamMode::None | BeamMode::ForwardHook | BeamMode::BackwardHook
            )
        {
            xml.end_element();
            open_beam = false;
        }
        let _ = position;
    }
    for _ in &open_tuplets {
        xml.end_element();
    }
    if open_beam {
        xml.end_element();
    }
    xml.end_element();
}

/// Chaîne des n-olets englobants, du plus externe au plus interne
/// (`Tuplet::parent` dans le modèle).
fn tuplet_chain(
    score: &Score,
    id: Option<scoretk_core::model::TupletId>,
) -> Vec<scoretk_core::model::TupletId> {
    let Some(mut id) = id else {
        return Vec::new();
    };
    let mut chain = vec![id];
    while let Some(parent) = score.tuplets[id.index()].parent {
        if chain.contains(&parent) {
            break;
        }
        chain.insert(0, parent);
        id = parent;
    }
    chain
}

/// `MeiExporter::writeArtics` (`meiexporter.cpp:1036`): articulations are
/// children of the note/chord event they belong to.
fn write_artics(xml: &mut XmlWriter, chord: &scoretk_core::model::Chord, full_support: bool) {
    let startid = chord.notes.first().map(|id| format!("#n{}", id.index()));
    if full_support {
        if chord.arpeggio.is_some() {
            xml.tag_empty("arpeg", &[]);
        }
        for ornament in &chord.ornaments {
            let type_name = format!("{}{:?}", ids::ORNAMENT_TYPE, ornament.kind);
            xml.tag_empty("ornam", &[("type", &type_name)]);
        }
    }
    for articulation in &chord.articulations {
        if full_support {
            let name = convert::artic_to_mei_full(&articulation.kind);
            let mut attrs = vec![("artic", name.as_str())];
            if let Some(startid) = &startid {
                // Ancrage exact : un `<chord>` écrit n'a pas d'`xml:id`, donc
                // le lecteur ne peut retrouver la portée que par une note.
                attrs.push(("startid", startid.as_str()));
            }
            xml.tag_empty("artic", &attrs);
        } else if let Some(name) = convert::artic_to_mei(&articulation.kind) {
            xml.tag_empty("artic", &[("artic", name)]);
        }
    }
}

/// `MeiExporter::writeNote` (`meiexporter.cpp:1370`).
/// Contexte de transcription d'une hauteur : transposition effective et
/// activation des porteurs additionnels.
#[derive(Clone, Copy)]
struct NotePitchContext {
    transpose: i32,
    full_support: bool,
}

fn write_note(
    xml: &mut XmlWriter,
    score: &Score,
    id: &scoretk_core::model::NoteId,
    extra: &[(&str, &str)],
    chord: Option<&scoretk_core::model::Chord>,
    kind: DurationType,
    pitch: NotePitchContext,
) {
    let note = &score.notes[id.index()];
    let (pname, oct, accid, accid_ges) = if pitch.full_support {
        let (pname, oct, accid) = convert::pitch_to_mei_written(note.pitch, pitch.transpose);
        (pname, oct, accid, None)
    } else {
        convert::pitch_to_mei(
            note.pitch,
            note.tpc,
            note.accidental.map(|a| a.kind),
            pitch.transpose,
        )
    };
    let mut attrs = extra.to_vec();
    let uid = format!("n{}", id.index());
    attrs.push(("xml:id", &uid));
    let pname_owned = pname.clone();
    attrs.push(("pname", &pname_owned));
    let oct_owned = oct.to_string();
    attrs.push(("oct", &oct_owned));
    if let Some(accid) = accid {
        attrs.push(("accid", accid));
    }
    if let Some(ges) = accid_ges {
        attrs.push(("accid.ges", ges));
    }
    // MEI 5.1 `att.noteHeads` : `@head.shape` (`data.HEADSHAPE`) quand le
    // terme est non ambigu, sinon `@glyph.name` avec `@glyph.auth="smufl"`.
    // `Convert::headShapeToMEI` regroupe par exemple slash, diamond old et
    // croix épaisse sous un même terme : le glyphe SMuFL les distingue.
    let shape = convert::head_shape_to_mei(&note.head_group)
        .filter(|shape| convert::head_shape_from_mei(shape) == Some(note.head_group.clone()));
    if let Some(shape) = shape {
        attrs.push(("head.shape", shape));
    } else if let Some(name) = convert::head_shape_to_smufl(&note.head_group, kind) {
        attrs.push(("glyph.auth", "smufl"));
        attrs.push(("glyph.name", name));
    }
    // L'élément reste ouvert : l'appelant y écrit les `<verse>` puis le ferme.
    xml.start_element("note", &attrs);
    if let Some(chord) = chord {
        write_artics(xml, chord, pitch.full_support);
    }
}

/// `MeiExporter::writeVerses`/`writeSyl` (`meiexporter.cpp:1579`, `:1529`).
fn write_verses(xml: &mut XmlWriter, cr: &ChordRest) {
    for lyric in &cr.lyrics {
        xml.start_element("verse", &[("n", &(lyric.verse + 1).to_string())]);
        let con = match lyric.syllabic {
            scoretk_core::model::LyricsSyllabic::Begin => Some("u"),
            scoretk_core::model::LyricsSyllabic::Middle => Some("m"),
            scoretk_core::model::LyricsSyllabic::End => Some("t"),
            scoretk_core::model::LyricsSyllabic::Single => None,
        };
        let attrs = con.map(|value| vec![("con", value)]).unwrap_or_default();
        xml.tag_with("syl", &attrs, &lyric.text);
        xml.end_element();
    }
}

/// Control events are written after the staves of a measure, as MuseScore
/// does (`writeMeasure` → `fillControlEventMap`).
fn write_control_events(
    xml: &mut XmlWriter,
    score: &Score,
    measure: &scoretk_core::model::Measure,
    index: usize,
    options: &MeiOptions,
) {
    // MuseScore ancre ses événements de contrôle sur les `xml:id` des
    // événements quand il peut (`findStart`, meiimporter.cpp:620). `@tstamp`
    // est une approximation continued-fraction et `tstamp_denominator` n'est
    // pas stable entre l'export 4.7.4 et sa relecture : l'ancrage par id est
    // exact, y compris quand l'extrémité tombe sur un silence.
    if options.full_support {
        for segment in &measure.segments {
            let Some(SegmentItem::Breath(breath)) = segment.element(Track::from_raw(0)) else {
                continue;
            };
            use scoretk_core::model::BreathType;
            let name = match breath.kind {
                BreathType::Caesura
                | BreathType::CaesuraCurved
                | BreathType::CaesuraShort
                | BreathType::CaesuraThick
                | BreathType::CaesuraSingle => "caesura",
                _ => "breath",
            };
            let type_name = format!("{}{:?}", ids::BREATH_TYPE, breath.kind);
            let stamp = convert::tstamp_from_fraction(
                segment.tick - measure.start_tick,
                convert::tstamp_denominator(measure),
            );
            let stamp = format!("{stamp:.3}");
            let attrs = [
                ("staff", "1"),
                ("tstamp", stamp.as_str()),
                ("type", type_name.as_str()),
            ];
            xml.tag_empty(name, &attrs);
        }
    }
    let event_at = event_ids(score, options.full_support);
    for segment in &measure.segments {
        for annotation in &segment.annotations {
            let staff = annotation.track.staff().raw() + 1;
            let stamp = convert::tstamp_from_fraction(
                segment.tick - measure.start_tick,
                convert::tstamp_denominator(measure),
            );
            let mut attrs = vec![
                ("staff".to_owned(), staff.to_string()),
                ("tstamp".to_owned(), format!("{stamp:.3}")),
            ];
            if options.full_support {
                let event = event_at
                    .get(&(segment.tick, annotation.track.raw()))
                    .or_else(|| {
                        event_at
                            .iter()
                            .find(|((tick, track), _)| {
                                *tick == segment.tick
                                    && scoretk_core::model::Track::from_raw(*track).staff()
                                        == annotation.track.staff()
                            })
                            .map(|(_, id)| id)
                    })
                    .cloned();
                if let Some(id) = event {
                    attrs.push(("startid".to_owned(), format!("#{id}")));
                }
            }
            let refs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            match &annotation.annotation {
                Annotation::Dynamic(dynamic) => {
                    let mut dynamic_attrs = refs.clone();
                    // MEI 5.1 `att.dynam.ges` → `att.midiValue/@val` : les
                    // Guidelines montrent `<dynam val="84">f</dynam>` pour
                    // une nuance MIDI (`cmn.html`, §"Dynamics"). MuseScore
                    // 4.7.4 n'écrit pas cette vitesse : porteur additionnel.
                    let velocity = options
                        .full_support
                        .then_some(dynamic.velocity)
                        .flatten()
                        .map(|value| value.to_string());
                    if let Some(velocity) = &velocity {
                        dynamic_attrs.push(("val", velocity.as_str()));
                    }
                    xml.tag_with("dynam", &dynamic_attrs, &dynamic.text);
                }
                Annotation::StaffText(text) | Annotation::Expression(text) => {
                    let mut type_attrs = refs.clone();
                    type_attrs.push(("type", "mscore-staff-text"));
                    xml.tag_with("dir", &type_attrs, text.text.plain());
                }
                Annotation::SystemText(text) => {
                    let mut type_attrs = refs.clone();
                    type_attrs.push(("type", "mscore-system-text"));
                    xml.tag_with("dir", &type_attrs, text.text.plain());
                }
                Annotation::Tempo(tempo) => {
                    let mut tempo_attrs = refs.clone();
                    let bpm = tempo.beats_per_second.map(|bps| (bps * 60.0).to_string());
                    if let Some(bpm) = &bpm {
                        tempo_attrs.push(("midi.bpm", bpm));
                    }
                    xml.tag_with("tempo", &tempo_attrs, tempo.text.plain());
                }
                Annotation::RehearsalMark(mark) => {
                    xml.tag_with("reh", &refs, mark.text.plain());
                }
                Annotation::Fermata(fermata) => {
                    let form = match fermata.kind {
                        scoretk_core::model::FermataType::Short
                        | scoretk_core::model::FermataType::VeryShort
                        | scoretk_core::model::FermataType::ShortHenze => "short",
                        scoretk_core::model::FermataType::Long
                        | scoretk_core::model::FermataType::VeryLong
                        | scoretk_core::model::FermataType::LongHenze => "long",
                        scoretk_core::model::FermataType::Curlew => "curlew",
                        scoretk_core::model::FermataType::Normal => "",
                    };
                    if form.is_empty() {
                        xml.tag_empty("fermata", &refs);
                    } else {
                        let mut fermata_attrs = refs.clone();
                        fermata_attrs.push(("form", form));
                        xml.tag_empty("fermata", &fermata_attrs);
                    }
                }
                Annotation::Harmony(harmony) => {
                    xml.tag_with("harm", &refs, &harmony.text);
                }
                Annotation::InstrumentChange(change) if options.full_support => {
                    let label = format!(
                        "{}{}",
                        ids::INSTRUMENT_CHANGE_LABEL,
                        change
                            .instrument
                            .as_ref()
                            .map_or("", |instrument| instrument.id.as_str())
                    );
                    let mut change_attrs = refs.clone();
                    change_attrs.push(("type", ids::INSTRUMENT_CHANGE_TYPE));
                    change_attrs.push(("label", label.as_str()));
                    let transpose = change
                        .instrument
                        .as_ref()
                        .and_then(|instrument| instrument.transpose);
                    let diatonic = transpose.map(|value| value.diatonic.to_string());
                    let chromatic = transpose.map(|value| value.chromatic.to_string());
                    if let Some(diatonic) = &diatonic {
                        change_attrs.push(("trans.diat", diatonic.as_str()));
                    }
                    if let Some(chromatic) = &chromatic {
                        change_attrs.push(("trans.semi", chromatic.as_str()));
                    }
                    xml.tag_with("dir", &change_attrs, change.text.plain());
                }
                _ => {}
            }
        }
    }
    // MuseScore anchors its control events on note ids (`@startid`/`@endid`),
    // which survives a tick that `@tstamp` can only approximate.
    for (id, spanner) in score.spanners.iter().enumerate() {
        if spanner.start.tick < measure.start_tick || spanner.start.tick >= measure.end_tick() {
            continue;
        }
        let den = convert::tstamp_denominator(measure);
        let start = convert::tstamp_from_fraction(spanner.start.tick - measure.start_tick, den);
        let end = convert::tstamp_from_fraction(
            (spanner.end.tick - measure.start_tick).max(Fraction::ZERO),
            den,
        );
        let start_note = event_at.get(&(spanner.start.tick, spanner.start.track.raw()));
        let end_note = event_at.get(&(spanner.end.tick, spanner.end.track.raw()));
        let mut attrs = vec![
            ("xml:id".to_owned(), format!("sp{id}")),
            (
                "staff".to_owned(),
                (spanner.start.track.staff().raw() + 1).to_string(),
            ),
        ];
        if let (Some(start_event), Some(end_event)) = (start_note, end_note) {
            attrs.push(("startid".to_owned(), format!("#{start_event}")));
            attrs.push(("endid".to_owned(), format!("#{end_event}")));
        } else {
            attrs.push(("tstamp".to_owned(), format!("{start:.3}")));
            attrs.push(("tstamp2".to_owned(), format!("{end:.3}")));
        }
        let mut refs: Vec<(&str, &str)> = attrs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        match &spanner.kind {
            SpannerType::Slur => {
                // `Convert::curvedirToMEI` (meiconverter.cpp:1123) : la
                // direction explicite est portée par `@curvedir` (MEI 5.1).
                if let Some(SpannerNotation::Slur(slur)) = &spanner.notation {
                    if let Some(dir) = convert::curvedir_to_mei(slur.placement) {
                        refs.push(("curvedir", dir));
                    }
                }
                xml.tag_empty("slur", &refs);
            }
            SpannerType::Hairpin(kind) => {
                let form = if *kind == scoretk_core::model::HairpinType::Diminuendo {
                    "dim"
                } else {
                    "cres"
                };
                let mut hairpin_attrs = refs.clone();
                hairpin_attrs.push(("form", form));
                xml.tag_empty("hairpin", &hairpin_attrs);
            }
            SpannerType::Pedal => {
                xml.tag_empty("pedal", &refs);
            }
            SpannerType::Ottava(_) => {
                xml.tag_empty("octave", &refs);
            }
            SpannerType::Trill => {
                xml.tag_empty("trill", &refs);
            }
            SpannerType::Glissando => {
                xml.tag_empty("gliss", &refs);
            }
            _ => {}
        }
    }
    // Ties are note-to-note in MEI (`startid`/`endid`), not a chord field.
    for (index, note) in score.notes.iter().enumerate() {
        if let Some(next) = note.tie_forward {
            let mut attrs = vec![
                ("startid".to_owned(), format!("#n{index}")),
                ("endid".to_owned(), format!("#n{}", next.index())),
            ];
            if let Some(dir) = convert::curvedir_to_mei(note.tie_up.map(|up| {
                if up {
                    scoretk_core::model::Placement::Above
                } else {
                    scoretk_core::model::Placement::Below
                }
            })) {
                attrs.push(("curvedir".to_owned(), dir.to_owned()));
            }
            let refs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            xml.tag_empty("tie", &refs);
        }
    }
    let _ = index;
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        ChordRest, Duration, Fraction, Measure, Note, Part, Score, Segment, SegmentType, Spanner,
        SpannerAnchor, SpannerType, StaffId, Track, VoltaInfo,
    };

    /// `<dynam @val>` est un porteur MEI 5.1 de la vitesse d'une nuance
    /// (`att.dynam.ges` → `att.midiValue`). MuseScore 4.7.4 ne l'écrit pas :
    /// il reste derrière `MeiOptions::full_support`.
    #[test]
    fn dynamic_velocity_is_an_additive_carrier() {
        use scoretk_core::model::{Annotation, Dynamic, Segment};

        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(
            scoretk_core::model::SegmentType::ChordRest,
            Fraction::ZERO,
            1,
        );
        let mut dynamic = Dynamic::new("f");
        dynamic.velocity = Some(96);
        segment.add_annotation(Track::new(staff, 0), Annotation::Dynamic(dynamic));
        measure.add_segment(segment);
        score.add_measure(measure);

        let default = write(&score, &MeiOptions::default()).expect("4.7.4 export");
        assert!(!default.contains("val=\"96\""));
        let full = write(
            &score,
            &MeiOptions {
                full_support: true,
                ..Default::default()
            },
        )
        .expect("extended export");
        assert!(full.contains("<dynam staff=\"1\" tstamp=\"1.000\" val=\"96\">f</dynam>"));
        let read_back = scoretk_mei_import::read_mei(full.as_bytes()).expect("read MEI");
        let items = scoretk_core::model::inventory(&read_back);
        assert!(items
            .items
            .iter()
            .any(|item| { item.kind == "dynamic-velocity" && item.value == "96" }));
    }

    /// Le type exact (huit variantes) et la durée d'un agrément sont portés
    /// par `@type="mscore-grace-*"`/`@dur` dans un `<graceGrp>` MEI 5.1.
    #[test]
    fn grace_chords_survive_an_extended_round_trip() {
        use scoretk_core::model::{
            Chord, ChordRestKind, DurationType, GraceChord, GraceType, Note,
        };

        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(
            scoretk_core::model::SegmentType::ChordRest,
            Fraction::ZERO,
            1,
        );
        let mut chord = ChordRest::chord(Duration::quarter());
        let main = score.add_note(Note::new(64, 16));
        let grace_note = score.add_note(Note::new(64, 18));
        let mut grace = GraceChord::new(
            GraceType::Grace32,
            Duration::new(DurationType::ThirtySecond),
        );
        grace.chord = Chord {
            grace: Some(GraceType::Grace32),
            notes: vec![grace_note],
            ..Default::default()
        };
        if let ChordRestKind::Chord(c) = &mut chord.kind {
            c.notes = vec![main];
            c.grace_chords = vec![grace];
        }
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
        score.add_measure(measure);

        let output = write(
            &score,
            &MeiOptions {
                full_support: true,
                ..Default::default()
            },
        )
        .expect("MEI with a grace note");
        assert!(output.contains("<graceGrp grace=\"acc\" type=\"mscore-grace-Grace32\""));
        let back = scoretk_mei_import::read_mei(output.as_bytes()).expect("read MEI");
        let items = scoretk_core::model::inventory(&back);
        assert!(items
            .items
            .iter()
            .any(|item| item.kind == "grace" && item.value == "Some(Grace32)"));
        assert!(items
            .items
            .iter()
            .any(|item| item.kind == "grace-note" && item.value == "64"));
    }

    /// MEI 5.1 n'a pas de cadre ancré sur une mesure : le `<dir>` additionnel
    /// conserve le texte, la mesure et la taille d'un `TBox` MuseScore.
    #[test]
    fn tbox_text_survives_an_extended_round_trip() {
        use scoretk_core::model::{Placement, RichText, TboxText};

        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        score.add_staff(part);
        score.add_measure(Measure::new(0, Fraction::ZERO, Fraction::new(4, 4)));
        let mut tbox = TboxText::new(RichText::new("Texte de cadre"), 0);
        tbox.font_size = Some(12.0);
        tbox.placement = Placement::Below;
        score.engraving.tbox_texts.push(tbox);

        let output = write(
            &score,
            &MeiOptions {
                full_support: true,
                ..Default::default()
            },
        )
        .expect("MEI with a text frame");
        assert!(output.contains("mscore-tbox-0"));
        let back = scoretk_mei_import::read_mei(output.as_bytes()).expect("read MEI");
        assert_eq!(back.engraving.tbox_texts.len(), 1);
        assert_eq!(back.engraving.tbox_texts[0].text.plain(), "Texte de cadre");
        assert_eq!(back.engraving.tbox_texts[0].placement, Placement::Below);
        assert_eq!(back.engraving.tbox_texts[0].font_size, Some(12.0));
    }

    #[test]
    fn manual_system_breaks_are_written() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        score.add_staff(part);
        score.add_measure(Measure::new(0, Fraction::ZERO, Fraction::new(4, 4)));
        let mut measure = Measure::new(1, Fraction::new(4, 4), Fraction::new(4, 4));
        measure.line_break = true;
        score.add_measure(measure);
        score.add_measure(Measure::new(2, Fraction::new(8, 4), Fraction::new(4, 4)));
        let output = write(&score, &MeiOptions::default()).expect("MEI with a system break");
        assert_eq!(output.matches("<sb type=\"mscore-manual\"").count(), 1);
    }

    #[test]
    fn tremolos_wrap_the_chord_event() {
        use scoretk_core::model::{Tremolo, TremoloKind};
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let track = Track::new(staff, 0);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        let mut chord = ChordRest::chord(Duration::quarter());
        let note = score.add_note(Note::new(60, 14));
        if let scoretk_core::model::ChordRestKind::Chord(c) = &mut chord.kind {
            c.notes = vec![note];
            c.tremolo = Some(Tremolo {
                kind: TremoloKind::Single,
                count: 1,
            });
        }
        segment.set_element(track, SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
        score.add_measure(measure);
        let output = write(&score, &MeiOptions::default()).expect("MEI with a tremolo");
        assert!(output.contains("<bTrem>"));
        assert!(output.contains("</bTrem>"));
    }

    #[test]
    fn signature_changes_are_written_as_score_def_changes() {
        use scoretk_core::model::TimeSig;
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let track = Track::new(staff, 0);
        let mut first = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(SegmentType::TimeSig, Fraction::ZERO, 1);
        segment.set_element(track, SegmentItem::TimeSig(TimeSig::new(4, 4)));
        first.add_segment(segment);
        score.add_measure(first);
        let mut second = Measure::new(1, Fraction::new(4, 4), Fraction::new(3, 4));
        let mut segment = Segment::new(SegmentType::TimeSig, Fraction::new(4, 4), 1);
        segment.set_element(track, SegmentItem::TimeSig(TimeSig::new(3, 4)));
        second.add_segment(segment);
        score.add_measure(second);
        let output = write(&score, &MeiOptions::default()).expect("MEI with a meter change");
        assert_eq!(output.matches("<scoreDef").count(), 2);
        assert!(output.contains("meter.count=\"3\""));
    }

    #[test]
    fn voltas_are_written_as_endings() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let track = Track::new(staff, 0);
        score.add_measure(Measure::new(0, Fraction::ZERO, Fraction::new(4, 4)));
        let mut spanner = Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(Fraction::ZERO, track),
            SpannerAnchor::new(Fraction::new(4, 4), track),
        );
        spanner.volta = Some(VoltaInfo {
            endings: vec![1, 2],
            closed: true,
        });
        score.add_spanner(spanner);
        let output = write(&score, &MeiOptions::default()).expect("MEI with a volta");
        assert!(output.contains("type=\"mscore-ending-1 mscore-ending-2\""));
    }

    #[test]
    fn spanners_are_anchored_on_note_ids() {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let track = Track::new(staff, 0);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        let mut chord = ChordRest::chord(Duration::quarter());
        let first = score.add_note(Note::new(60, 14));
        let second = score.add_note(Note::new(62, 16));
        if let scoretk_core::model::ChordRestKind::Chord(c) = &mut chord.kind {
            c.notes = vec![first];
        }
        segment.set_element(track, SegmentItem::ChordRest(chord));
        let mut segment2 = Segment::new(SegmentType::ChordRest, Fraction::new(1, 4), 1);
        let mut chord2 = ChordRest::chord(Duration::quarter());
        if let scoretk_core::model::ChordRestKind::Chord(c) = &mut chord2.kind {
            c.notes = vec![second];
        }
        segment2.set_element(track, SegmentItem::ChordRest(chord2));
        measure.add_segment(segment);
        measure.add_segment(segment2);
        score.add_measure(measure);
        score.add_spanner(Spanner::new(
            SpannerType::Slur,
            SpannerAnchor::new(Fraction::ZERO, track),
            SpannerAnchor::new(Fraction::new(1, 4), track),
        ));
        let output = write(&score, &MeiOptions::default()).expect("MEI with a slur");
        assert!(output.contains("startid=\"#n0\""));
        assert!(output.contains("endid=\"#n1\""));
    }

    /// MEI 5.1 `model.noteLike` : `<verse>` est un enfant de `<note>` (ou de
    /// `<chord>`), jamais un frère dans la couche. `MeiExporter::writeVerse`
    /// (meiexporter.cpp:1594) l'attache au nœud courant avant d'écrire la note.
    #[test]
    fn verses_are_children_of_the_note_and_survive_a_read() {
        use scoretk_core::model::{ChordRestKind, Lyrics, LyricsSyllabic};

        let mut score = Score::new();
        let part = score.add_part(Part::new("Voice", "Vox"));
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        let mut chord = ChordRest::chord(Duration::quarter());
        let note = score.add_note(Note::new(60, 14));
        if let ChordRestKind::Chord(c) = &mut chord.kind {
            c.notes = vec![note];
        }
        chord
            .lyrics
            .push(Lyrics::new(0, LyricsSyllabic::Single, "Du"));
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
        score.add_measure(measure);

        let output = write(&score, &MeiOptions::default()).expect("MEI with a lyric");
        fn find<'a>(
            node: &'a scoretk_xml_common::Element,
            name: &str,
        ) -> Option<&'a scoretk_xml_common::Element> {
            if node.name == name {
                return Some(node);
            }
            node.children.iter().find_map(|child| find(child, name))
        }
        let tree = scoretk_xml_common::parse(output.as_bytes()).expect("XML");
        let note = find(&tree, "note").expect("note element");
        assert_eq!(
            note.child("verse")
                .and_then(|verse| verse.child_text("syl")),
            Some("Du")
        );
        let read_back = scoretk_mei_import::read_mei(output.as_bytes()).expect("read MEI");
        let lyric = &read_back.measures[0]
            .segments
            .iter()
            .find_map(|segment| {
                segment
                    .element(Track::new(StaffId::from_index(0), 0))
                    .and_then(SegmentItem::as_chord_rest)
            })
            .expect("chord rest")
            .lyrics[0];
        assert_eq!(lyric.text, "Du");
    }

    /// `Convert::staffToMEI` (meiconverter.cpp:3099) et `pitchToMEI` (`:2820`)
    /// : la hauteur `__pitch__` du modèle est réelle, le TPC est la note
    /// *écrite*. Sans `@trans.diat`/`@trans.semi` sur `<staffDef>`, la
    /// relecture transposerait une seconde fois (Handel/Hummel/Berlioz).
    #[test]
    fn transposing_instruments_write_their_written_pitch() {
        use scoretk_core::model::Transpose;

        let mut score = Score::new();
        let mut part = Part::new("Clarinet", "Cl");
        part.instrument.transpose = Some(Transpose::new(2, 2));
        let part_id = score.add_part(part);
        let staff = score.add_staff_with(part_id, scoretk_core::model::Staff::new(part_id));
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        let mut chord = ChordRest::chord(Duration::quarter());
        let note = score.add_note(Note::new(64, 16)); // concert E4, écrit D4
        if let scoretk_core::model::ChordRestKind::Chord(c) = &mut chord.kind {
            c.notes = vec![note];
        }
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
        score.add_measure(measure);

        let output = write(&score, &MeiOptions::default()).expect("MEI with transposition");
        assert!(output.contains("trans.diat=\"2\""), "{output}");
        assert!(output.contains("trans.semi=\"2\""), "{output}");
        assert!(output.contains("pname=\"d\" oct=\"4\""), "{output}");
        let read_back = scoretk_mei_import::read_mei(output.as_bytes()).expect("read MEI");
        assert_eq!(read_back.notes[0].pitch, 64);
    }
}
