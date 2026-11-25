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

use scoretk_core::model::{
    Annotation, ChordRest, ChordRestKind, Fraction, Score, SegmentItem, SpannerType, Track, VOICES,
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
    header::write_score_def(&mut xml, score);
    xml.start_element("section", &[("xml:id", "s1")]);
    let mut measure_n = 0;
    for (index, measure) in score.measures.iter().enumerate() {
        write_measure(&mut xml, score, measure, index, &mut measure_n, options);
    }
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    Ok(xml.finish())
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
    if options.layout && measure.section_break_before {
        xml.tag_empty("pb", &[("type", ids::BREAK_TYPE)]);
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
            write_layer(xml, score, measure, track, voice, index);
        }
        xml.end_element();
    }
    write_control_events(xml, score, measure, index);
    xml.end_element();
}

/// `MeiExporter::writeLayer` (`meiexporter.cpp:964`).
fn write_layer(
    xml: &mut XmlWriter,
    score: &Score,
    measure: &scoretk_core::model::Measure,
    track: Track,
    voice: u8,
    measure_index: usize,
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
    xml.start_element("layer", &[("xml:id", &layer_id), ("n", &layer_n)]);
    let mut open_tuplet: Option<scoretk_core::model::TupletId> = None;
    for (position, (tick, cr)) in events.iter().enumerate() {
        let mut attributes = Vec::new();
        attributes.push((
            "dur".to_owned(),
            convert::dur_to_mei(cr.duration.kind).to_owned(),
        ));
        if cr.duration.dots > 0 {
            attributes.push(("dots".to_owned(), cr.duration.dots.to_string()));
        }
        let beam = convert::beam_to_mei(cr.engraving.beam_mode, ids::BEAM_ELEMENT_TYPE);
        if !beam.is_empty() {
            attributes.push(("type".to_owned(), beam));
        }
        if let Some(breaksec) = convert::breaksec_to_mei(cr.engraving.beam_mode) {
            attributes.push(("breaksec".to_owned(), breaksec.to_string()));
        }
        if cr.tuplet != open_tuplet {
            if let Some(id) = open_tuplet {
                let _ = id;
                xml.end_element();
            }
            open_tuplet = cr.tuplet;
            if let Some(id) = cr.tuplet {
                let tuplet = &score.tuplets[id.index()];
                let attrs = [
                    ("num".to_owned(), tuplet.actual_notes.to_string()),
                    ("numbase".to_owned(), tuplet.normal_notes.to_string()),
                ];
                let refs: Vec<(&str, &str)> = attrs
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                xml.start_element("tuplet", &refs);
            }
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
        match &cr.kind {
            ChordRestKind::Rest(rest) if rest.full_measure => {
                xml.tag_empty("mRest", &[("xml:id", &format!("mr{measure_index}"))]);
            }
            ChordRestKind::Rest(rest) => {
                let mut rest_attrs = refs.clone();
                if !rest.visible {
                    rest_attrs.push(("visible", "false"));
                }
                xml.start_element("rest", &rest_attrs);
                write_verses(xml, cr);
                xml.end_element();
            }
            ChordRestKind::Chord(chord) => {
                if chord.notes.len() == 1 {
                    write_note(xml, score, &chord.notes[0], &refs);
                    write_verses(xml, cr);
                } else {
                    xml.start_element("chord", &refs);
                    for id in &chord.notes {
                        write_note(xml, score, id, &[]);
                    }
                    write_verses(xml, cr);
                    xml.end_element();
                }
            }
        }
        let _ = position;
    }
    if open_tuplet.is_some() {
        xml.end_element();
    }
    xml.end_element();
}

/// `MeiExporter::writeNote` (`meiexporter.cpp:1370`).
fn write_note(
    xml: &mut XmlWriter,
    score: &Score,
    id: &scoretk_core::model::NoteId,
    extra: &[(&str, &str)],
) {
    let note = &score.notes[id.index()];
    let (pname, oct, accid, accid_ges) =
        convert::pitch_to_mei(note.pitch, note.tpc, note.accidental.map(|a| a.kind), 0);
    let mut attrs = extra.to_vec();
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
    xml.tag_empty("note", &attrs);
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
) {
    for segment in &measure.segments {
        for annotation in &segment.annotations {
            let staff = annotation.track.staff().raw() + 1;
            let stamp = convert::tstamp_from_fraction(
                segment.tick - measure.start_tick,
                convert::tstamp_denominator(measure),
            );
            let attrs = [
                ("staff".to_owned(), staff.to_string()),
                ("tstamp".to_owned(), format!("{stamp:.3}")),
            ];
            let refs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            match &annotation.annotation {
                Annotation::Dynamic(dynamic) => {
                    xml.tag_with("dynam", &refs, &dynamic.text);
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
                Annotation::Harmony(harmony) => {
                    xml.tag_with("harm", &refs, &harmony.text);
                }
                _ => {}
            }
        }
    }
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
        let attrs = [
            ("xml:id".to_owned(), format!("sp{id}")),
            (
                "staff".to_owned(),
                (spanner.start.track.staff().raw() + 1).to_string(),
            ),
            ("tstamp".to_owned(), format!("{start:.3}")),
            ("tstamp2".to_owned(), format!("{end:.3}")),
        ];
        let refs: Vec<(&str, &str)> = attrs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        match &spanner.kind {
            SpannerType::Slur => {
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
    let _ = index;
}
