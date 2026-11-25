// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiimporter.cpp` de MuseScore Studio
// 4.7.4, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026. Les noms des méthodes suivent le C++ (`readScore`, `readScoreDef`,
// `readMeasure`, `readLayers`, `readElements`, `addChordRest`, …).
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

//! Structure du lecteur : `readScore`, `readScoreDef`, `readMeasure`.

use std::collections::BTreeMap;

use scoretk_core::model::{
    Accidental, BeamMode, ChordRest, ChordRestKind, Clef, ClefType, Duration, Fraction, Key,
    Layers, Measure, Note, NoteId, Segment, SegmentItem, SegmentType, StaffId, TimeSig,
    TimeSigType, Track, Transpose, Tuplet, VOICES,
};
use scoretk_mei_common::convert;
use scoretk_xml_common::Element;

use crate::{first_descendant, Error};

/// Key of global `scoreDef` values in the signature maps.
const SCOREDEF_IDX: usize = usize::MAX;

/// Spanner en attente d'une extrémité (`addSpanner` / `findEnd`).
#[derive(Clone, Debug)]
pub(crate) struct PendingSpanner {
    pub(crate) kind: scoretk_core::model::SpannerType,
    pub(crate) number: u8,
    pub(crate) start: (Fraction, Track),
    pub(crate) end_id: Option<String>,
    pub(crate) end_tstamp: Option<f64>,
    pub(crate) measure: usize,
}

pub(crate) struct Reader<'a> {
    pub(crate) score: scoretk_core::model::Score,
    pub(crate) root: &'a Element,
    /// `staff@n` → index de portée.
    pub(crate) staff_by_n: BTreeMap<i32, usize>,
    pub(crate) part_by_staff: BTreeMap<i32, usize>,
    pub(crate) clefs: BTreeMap<usize, ClefType>,
    pub(crate) timesigs: BTreeMap<usize, (i32, i32, TimeSigType)>,
    pub(crate) keysigs: BTreeMap<usize, Key>,
    pub(crate) transpose: BTreeMap<usize, Transpose>,
    /// `xml:id` → note, pour les liaisons.
    pub(crate) id_notes: BTreeMap<String, NoteId>,
    /// `xml:id` → (mesure, piste, tick), pour les événements de contrôle.
    pub(crate) id_events: BTreeMap<String, (usize, Track, Fraction)>,
    pub(crate) pending: Vec<PendingSpanner>,
    /// Ties whose end note is in a later measure.
    pub(crate) pending_ties: Vec<(String, String)>,
    pub(crate) current_time: Fraction,
    pub(crate) tuplet_ratio: Option<Fraction>,
    pub(crate) beam_mode: BeamMode,
    /// Signatures must be written in the next measure (initial `scoreDef` or
    /// a `scoreDef` change inside a section).
    pub(crate) signatures_pending: bool,
}

impl<'a> Reader<'a> {
    /// `MeiImporter::read` → `readScore`.
    pub(crate) fn read(root: &'a Element) -> Result<scoretk_core::model::Score, Error> {
        let Some(score_node) = first_descendant(root, "score") else {
            return Err(Error::NoScore);
        };
        let mut reader = Self {
            score: scoretk_core::model::Score::new(),
            root,
            staff_by_n: BTreeMap::new(),
            part_by_staff: BTreeMap::new(),
            clefs: BTreeMap::new(),
            timesigs: BTreeMap::new(),
            keysigs: BTreeMap::new(),
            transpose: BTreeMap::new(),
            id_notes: BTreeMap::new(),
            id_events: BTreeMap::new(),
            pending: Vec::new(),
            pending_ties: Vec::new(),
            current_time: Fraction::new(4, 4),
            tuplet_ratio: None,
            beam_mode: BeamMode::Auto,
            signatures_pending: true,
        };
        reader.build_staff_layer_map(score_node);
        reader.read_mei_head();
        reader.read_score(score_node)?;
        reader.add_spanner_ends();
        crate::barlines::finalize(&mut reader.score);
        Ok(reader.score)
    }

    /// `MeiImporter::buildStaffLayerMap` (`meiimporter.cpp:3309`).
    fn build_staff_layer_map(&mut self, score_node: &Element) {
        let Some(def) = first_descendant(score_node, "scoreDef") else {
            return;
        };
        let mut index = 0;
        for staff_def in descendants_named(def, "staffDef") {
            let n = staff_def
                .attribute("n")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(index as i32 + 1);
            if self.staff_by_n.insert(n, index).is_none() {
                index += 1;
            }
        }
    }

    /// `MeiImporter::readScore` (`meiimporter.cpp:1011`).
    fn read_score(&mut self, score_node: &Element) -> Result<(), Error> {
        let score_def = score_node.child("scoreDef");
        if let Some(score_def) = score_def {
            self.read_score_def(score_def, true);
        }
        for section in score_node.children_named("section") {
            self.read_section_elements(section);
        }
        Ok(())
    }

    /// `MeiImporter::readSectionElements` (`meiimporter.cpp:1346`).
    pub(crate) fn read_section_elements(&mut self, parent: &Element) {
        for child in &parent.children {
            match child.name.as_str() {
                "scoreDef" => self.read_score_def(child, false),
                "section" => self.read_section_elements(child),
                "measure" => {
                    self.read_measure(child);
                    // MuseScore 4.7.4 can write a measure inside another one;
                    // read those too, as the reference model has them all.
                    for nested in child.children_named("measure") {
                        self.read_measure(nested);
                    }
                }
                "ending" => {
                    let endings = child
                        .attribute("n")
                        .unwrap_or("1")
                        .split(',')
                        .filter_map(|value| value.trim().parse::<u8>().ok())
                        .collect::<Vec<_>>();
                    let first = self.score.measures.len();
                    for measure in child.children_named("measure") {
                        self.read_measure(measure);
                    }
                    let last = self.score.measures.len();
                    if last > first {
                        let start = self.score.measures[first].start_tick;
                        let end = self.score.measures[last - 1].end_tick();
                        // MuseScore attaches the volta to the staff of the
                        // first measure; keep the same anchor.
                        let track = Track::new(StaffId::from_index(0), 0);
                        let mut spanner = scoretk_core::model::Spanner::new(
                            scoretk_core::model::SpannerType::Volta,
                            scoretk_core::model::SpannerAnchor::new(start, track),
                            scoretk_core::model::SpannerAnchor::new(end, track),
                        );
                        spanner.volta = Some(scoretk_core::model::VoltaInfo {
                            endings,
                            closed: true,
                        });
                        self.score.add_spanner(spanner);
                    }
                }
                "pb" if child.attribute("type") == Some("mscore-manual") => {
                    if let Some(measure) = self.score.measures.last_mut() {
                        measure.section_break_before = true;
                    }
                }
                "sb" if child.attribute("type") == Some("mscore-manual") => {
                    if let Some(measure) = self.score.measures.last_mut() {
                        measure.line_break = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// `MeiImporter::readMeasure` (`meiimporter.cpp:1434`).
    fn read_measure(&mut self, node: &Element) {
        let info = convert::measure_from_mei(
            node.attribute("n"),
            node.attribute("left"),
            node.attribute("right"),
            node.attribute("type"),
        );
        let index = self.score.measures.len();
        let (numerator, denominator, kind) = self.current_timesig();
        let nominal = Fraction::try_new(numerator, denominator).unwrap_or(Fraction::new(4, 4));
        let start = self.score.end_tick();
        let mut measure = Measure::new(index, start, nominal);
        measure.time_sig_numerator = numerator;
        measure.number_offset = if info.irregular {
            0
        } else {
            info.n as i32 - 1 - index as i32
        };
        if info.irregular {
            measure.exclude_from_numbering = true;
        }
        measure.start_bar_line = if info.repeat_start {
            scoretk_core::model::BarLineType::StartRepeat
        } else {
            scoretk_core::model::BarLineType::Normal
        };
        measure.end_bar_line = if info.repeat_end {
            scoretk_core::model::BarLineType::EndRepeat
        } else {
            info.end_bar_line
        };
        measure.repeat_count = info.repeat_count;

        // Initial signatures of every staff, only in the first measure after
        // a scoreDef (MuseScore writes them once, not in every measure).
        if self.signatures_pending {
            let staff_count = self.score.staves.len().max(1);
            for staff_idx in 0..staff_count {
                if let Some(clef) = self.clefs.get(&staff_idx).copied() {
                    Self::insert_segment(
                        &mut measure,
                        staff_count,
                        Fraction::ZERO,
                        SegmentType::Clef,
                        staff_idx,
                        SegmentItem::Clef(Clef::new(clef)),
                    );
                }
                if let Some(key) = self.keysigs.get(&staff_idx).copied() {
                    Self::insert_segment(
                        &mut measure,
                        staff_count,
                        Fraction::ZERO,
                        SegmentType::KeySig,
                        staff_idx,
                        SegmentItem::KeySig(scoretk_core::model::KeySig::new(key)),
                    );
                }
                Self::insert_segment(
                    &mut measure,
                    staff_count,
                    Fraction::ZERO,
                    SegmentType::TimeSig,
                    staff_idx,
                    SegmentItem::TimeSig(TimeSig {
                        numerator: numerator as i32,
                        denominator: denominator as i32,
                        kind,
                        ..TimeSig::new(numerator as i32, denominator as i32)
                    }),
                );
            }
            self.signatures_pending = false;
        }

        self.score.add_measure(measure);
        self.read_staves(node, index);
        self.read_control_events(node, index);
        // Several staves may share the last tick: the measure length is the
        // longest voice, not the sum of all tracks.
        let len = self.score.measures[index]
            .last_segment()
            .map_or(Fraction::ZERO, |segment| {
                segment
                    .iter_elements()
                    .filter_map(|(_, item)| item.as_chord_rest())
                    .map(|cr| cr.ticks_in_measure(nominal))
                    .max()
                    .unwrap_or(Fraction::ZERO)
                    + (segment.tick - start)
            });
        // The MuseScore `.mscx` reader defaults `Measure::len` to the nominal
        // length; only an irregular measure (`@metcon="false"` or no `@n`)
        // carries its real length. Match that so ticks stay aligned.
        let irregular = info.irregular || node.attribute("metcon") == Some("false");
        let gap = info.n.saturating_sub(index + 1) as i64;
        self.score.measures[index].len = if node.attribute("metcon") == Some("false") && gap > 0 {
            nominal * Fraction::from_integer(gap)
        } else if irregular && len > Fraction::ZERO {
            len
        } else {
            nominal
        };
        if gap > 0 {
            self.score.measures[index].multi_measure_rest = Some(gap as u16);
        }
    }

    /// Insert (or complete) a segment at `tick` for `staff_idx`.
    pub(crate) fn insert_segment(
        measure: &mut Measure,
        staff_count: usize,
        tick: Fraction,
        kind: SegmentType,
        staff_idx: usize,
        item: SegmentItem,
    ) {
        let position = measure
            .segments
            .iter()
            .position(|segment| segment.tick == tick && segment.kind == kind);
        let track = Track::new(StaffId::from_index(staff_idx), 0);
        if let Some(position) = position {
            measure.segments[position].set_element(track, item);
        } else {
            let mut segment = Segment::new(kind, tick, staff_count.max(1));
            segment.set_element(track, item);
            measure.add_segment(segment);
        }
    }

    /// `MeiImporter::readStaves` (`meiimporter.cpp:1544`).
    fn read_staves(&mut self, parent: &Element, measure: usize) {
        for (order, staff) in parent.children_named("staff").enumerate() {
            let n = staff
                .attribute("n")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(order as i32 + 1);
            let staff_idx = self.staff_by_n.get(&n).copied().unwrap_or(order);
            for layer in staff.children_named("layer") {
                let layer_n = layer
                    .attribute("n")
                    .and_then(|value| value.parse::<u8>().ok())
                    .unwrap_or(1);
                if layer_n == 0 || layer_n as usize > VOICES {
                    self.score
                        .report
                        .record(Layers::EXTENSIONS, "layer-beyond-voices", None, None);
                    continue;
                }
                let track = Track::new(StaffId::from_index(staff_idx), layer_n - 1);
                let mut ticks = Fraction::ZERO;
                self.read_elements(layer, measure, track, &mut ticks);
            }
        }
    }

    /// `MeiImporter::readElements` (`meiimporter.cpp:1654`).
    pub(crate) fn read_elements(
        &mut self,
        parent: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        for node in &parent.children {
            match node.name.as_str() {
                "beam" => self.read_beam(node, measure, track, ticks),
                "chord" => {
                    self.read_chord(node, measure, track, ticks);
                }
                "note" => {
                    self.read_note(node, measure, track, ticks);
                }
                "rest" => self.read_rest(node, measure, track, ticks),
                "space" => self.read_space(node, measure, track, ticks),
                "mRest" => self.read_m_rest(node, measure, track, ticks),
                "mRpt" => self.read_m_rpt(node, measure, track, ticks),
                "tuplet" => self.read_tuplet(node, measure, track, ticks),
                "clef" => self.read_clef_element(node, measure, track, ticks),
                "bTrem" | "fTrem" => self.read_tremolo(node, measure, track, ticks),
                "graceGrp" => self.read_grace_group(node, measure, track, ticks),
                "accid" | "artic" | "arpeg" | "fermata" | "fing" | "dir" | "dynam" | "reh"
                | "harm" | "trill" | "mordent" | "turn" | "ornam" => {
                    self.read_control_events(node, measure);
                }
                _ => {}
            }
        }
    }

    /// `MeiImporter::readBeam` (`meiimporter.cpp:1784`). The 4.7.4 importer
    /// relies on automatic beaming for a `<beam>` container; only the
    /// `mscore-beam-*` `@type` values set an explicit mode.
    fn read_beam(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let previous = self.beam_mode;
        self.read_elements(node, measure, track, ticks);
        self.beam_mode = previous;
    }

    /// `MeiImporter::addChordRest` (`meiimporter.cpp:242`).
    pub(crate) fn add_chord_rest(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: Fraction,
        duration: Duration,
        is_rest: bool,
    ) -> usize {
        let start = self.score.measures[measure].start_tick;
        let absolute_for_index = start + ticks;
        let mut cr = if is_rest {
            ChordRest::rest(duration)
        } else {
            ChordRest::chord(duration)
        };
        cr.ticks = Some(duration.ticks());
        if let Some(stem) = convert::stem_from_mei(node.attribute("stem.dir").unwrap_or_default()) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.engraving.stem = stem;
            }
        }
        let beam =
            convert::beam_from_mei(node.attribute("type").unwrap_or_default(), "mscore-beam");
        if beam != BeamMode::Auto {
            cr.engraving.beam_mode = beam;
        } else if let Some(breaksec) = node
            .attribute("breaksec")
            .and_then(|v| v.parse::<i32>().ok())
        {
            cr.engraving.beam_mode = convert::breaksec_from_mei(breaksec);
        }
        let absolute = start + ticks;
        let segment = Segment::new(
            SegmentType::ChordRest,
            absolute,
            self.score.staves.len().max(1),
        );
        let mut segment = segment;
        segment.set_element(track, SegmentItem::ChordRest(cr));
        let position = self.score.measures[measure]
            .segments
            .iter()
            .position(|existing| {
                existing.tick == absolute && existing.kind == SegmentType::ChordRest
            });
        let position = match position {
            Some(position) => {
                self.score.measures[measure].segments[position].set_element(
                    track,
                    segment.element(track).cloned().expect("new chord rest"),
                );
                position
            }
            None => {
                self.score.measures[measure].add_segment(segment);
                self.score.measures[measure]
                    .segments
                    .iter()
                    .position(|existing| {
                        existing.tick == absolute_for_index
                            && existing.kind == SegmentType::ChordRest
                    })
                    .expect("inserted chord rest segment")
            }
        };
        let id = node.attribute("id").map(str::to_owned);
        if let Some(id) = id {
            self.id_events.insert(id, (measure, track, absolute));
        }
        position
    }

    /// `MeiImporter::readNote` (`meiimporter.cpp:2040`).
    fn read_note(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, false);
        let accid_node = first_descendant(node, "accid");
        let accid = accid_node.unwrap_or(node);
        let staff_idx = track.staff().raw() as usize;
        let transpose = self.transpose.get(&staff_idx).copied().unwrap_or_default();
        let info = convert::pitch_from_mei(
            node.attribute("pname"),
            node.attribute("oct").and_then(|value| value.parse().ok()),
            accid.attribute("accid"),
            accid.attribute("accid.ges"),
            transpose.chromatic,
        );
        let mut note = Note::new(info.pitch, info.tpc);
        note.accidental = info.accidental.map(|kind| Accidental {
            kind,
            bracket: accid.attribute("enclose") == Some("brack"),
            parentheses: accid.attribute("enclose") == Some("paren"),
            visible: node.attribute("visible") != Some("false"),
            user: accid.attribute("accid").is_some(),
            small: false,
        });
        if let Some(vel) = node
            .attribute("vel")
            .and_then(|value| value.parse::<u8>().ok())
        {
            note.playback.user_velocity = Some(i32::from(vel));
        }
        let note_id = self.score.add_note(note);
        if let Some(id) = node.attribute("id") {
            self.id_notes.insert(id.to_owned(), note_id);
        }
        if let Some(SegmentItem::ChordRest(cr)) =
            self.score.measures[measure].segments[index].element_mut(track)
        {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.notes.push(note_id);
            }
        }
        self.read_verses(node, measure, track, ticks);
        self.advance(ticks, duration, measure);
    }

    /// `MeiImporter::readChord` (`meiimporter.cpp:1835`).
    fn read_chord(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, false);
        let staff_idx = track.staff().raw() as usize;
        let transpose = self.transpose.get(&staff_idx).copied().unwrap_or_default();
        let mut notes = Vec::new();
        for note_node in node.children_named("note") {
            let accid = first_descendant(note_node, "accid").unwrap_or(note_node);
            let info = convert::pitch_from_mei(
                note_node.attribute("pname"),
                note_node
                    .attribute("oct")
                    .and_then(|value| value.parse().ok()),
                accid.attribute("accid"),
                accid.attribute("accid.ges"),
                transpose.chromatic,
            );
            let mut note = Note::new(info.pitch, info.tpc);
            note.accidental = info.accidental.map(|kind| Accidental {
                kind,
                bracket: accid.attribute("enclose") == Some("brack"),
                parentheses: accid.attribute("enclose") == Some("paren"),
                visible: note_node.attribute("visible") != Some("false"),
                user: accid.attribute("accid").is_some(),
                small: false,
            });
            let note_id = self.score.add_note(note);
            if let Some(id) = note_node.attribute("id") {
                self.id_notes.insert(id.to_owned(), note_id);
            }
            notes.push(note_id);
        }
        let segment = &mut self.score.measures[measure].segments[index];
        if let Some(SegmentItem::ChordRest(cr)) = segment.element_mut(track) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord.notes = notes;
            }
        }
        self.read_verses(node, measure, track, ticks);
        self.advance(ticks, duration, measure);
    }

    /// `MeiImporter::readRest` (`meiimporter.cpp:2139`).
    fn read_rest(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, true);
        if node.attribute("visible") == Some("false") {
            if let Some(SegmentItem::ChordRest(cr)) =
                self.score.measures[measure].segments[index].element_mut(track)
            {
                if let ChordRestKind::Rest(rest) = &mut cr.kind {
                    rest.visible = false;
                }
            }
        }
        self.advance(ticks, duration, measure);
    }

    /// `MeiImporter::readSpace` (`meiimporter.cpp:2160`): an invisible rest.
    fn read_space(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let duration = duration_of(node);
        let index = self.add_chord_rest(node, measure, track, *ticks, duration, true);
        if let Some(SegmentItem::ChordRest(cr)) =
            self.score.measures[measure].segments[index].element_mut(track)
        {
            if let ChordRestKind::Rest(rest) = &mut cr.kind {
                rest.gap = true;
                rest.visible = false;
            }
        }
        self.advance(ticks, duration, measure);
    }

    /// `MeiImporter::readMRest` (`meiimporter.cpp:1972`).
    fn read_m_rest(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        self.add_chord_rest(node, measure, track, *ticks, Duration::measure(), true);
        let nominal = self.score.measures[measure].nominal_len;
        if let Some(cr) = self.last_chord_rest_mut(measure, track) {
            cr.ticks = Some(nominal);
        }
        *ticks += nominal;
    }

    /// `MeiImporter::readMRpt` (`meiimporter.cpp:2010`).
    fn read_m_rpt(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let nominal = self.score.measures[measure].nominal_len;
        self.add_chord_rest(node, measure, track, *ticks, Duration::whole(), true);
        self.score.measures[measure].measure_repeat =
            Some(scoretk_core::model::MeasureRepeatMark {
                staff: StaffId::from_index(track.staff().raw() as usize),
                count: 1_u16,
            });
        *ticks += nominal;
    }

    /// `MeiImporter::readTuplet` (`meiimporter.cpp:2200`). Nested tuplets are
    /// rejected by MuseScore 4.7.4.
    fn read_tuplet(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        if self.tuplet_ratio.is_some() {
            self.score.report.record(
                Layers::EXTENSIONS,
                "nested-tuplet",
                Some(self.score.measures[measure].start_tick + *ticks),
                Some(track),
            );
            self.read_elements(node, measure, track, ticks);
            return;
        }
        let num = node
            .attribute("num")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(1);
        let base = node
            .attribute("numbase")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(1);
        let ratio = Fraction::try_new(num, base).unwrap_or(Fraction::ONE);
        let start_tick = self.score.measures[measure].start_tick + *ticks;
        let before = *ticks;
        self.tuplet_ratio = Some(ratio);
        self.read_elements(node, measure, track, ticks);
        self.tuplet_ratio = None;
        let base_len = node
            .children
            .iter()
            .find_map(duration_of_opt)
            .map(|duration| duration.ticks())
            .unwrap_or(Fraction::QUARTER);
        let mut tuplet = Tuplet::new(ratio, base_len, start_tick, *ticks - before);
        tuplet.bracket = node
            .attribute("bracket.visible")
            .is_some_and(|value| value != "false");
        let tuplet_id = self.score.add_tuplet(tuplet);
        for segment in &mut self.score.measures[measure].segments {
            if segment.tick < start_tick || segment.tick >= start_tick + (*ticks - before) {
                continue;
            }
            if let Some(item) = segment.element_mut(track) {
                if let Some(cr) = item.as_chord_rest_mut() {
                    cr.tuplet = Some(tuplet_id);
                }
            }
        }
    }

    /// `MeiImporter::readClef` (`meiimporter.cpp:1879`).
    fn read_clef_element(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        let clef = convert::clef_from_mei(
            node.attribute("shape"),
            node.attribute("line")
                .and_then(|value| value.parse().ok())
                .unwrap_or(2),
            node.attribute("dis"),
            node.attribute("dis.place"),
        );
        let tick = self.score.measures[measure].start_tick + *ticks;
        let staff_count = self.score.staves.len();
        Self::insert_segment(
            &mut self.score.measures[measure],
            staff_count,
            tick,
            SegmentType::Clef,
            track.staff().raw() as usize,
            SegmentItem::Clef(Clef::new(clef)),
        );
    }

    /// `MeiImporter::readBTrem`/`readFTrem` (`meiimporter.cpp:1812`, `:1910`).
    fn read_tremolo(&mut self, node: &Element, measure: usize, track: Track, ticks: &mut Fraction) {
        let before = self.score.measures[measure].start_tick + *ticks;
        self.read_elements(node, measure, track, ticks);
        for segment in &mut self.score.measures[measure].segments {
            if segment.tick < before {
                continue;
            }
            if let Some(SegmentItem::ChordRest(cr)) = segment.element_mut(track) {
                if let ChordRestKind::Chord(chord) = &mut cr.kind {
                    chord.tremolo = Some(scoretk_core::model::Tremolo {
                        kind: scoretk_core::model::TremoloKind::Single,
                        count: 1,
                    });
                }
            }
        }
    }

    /// `MeiImporter::readGraceGrp` (`meiimporter.cpp:1938`).
    fn read_grace_group(
        &mut self,
        node: &Element,
        measure: usize,
        track: Track,
        ticks: &mut Fraction,
    ) {
        let before = *ticks;
        self.read_elements(node, measure, track, ticks);
        *ticks = before;
        if let Some(cr) = self.last_chord_rest_mut(measure, track) {
            if let ChordRestKind::Chord(chord) = &mut cr.kind {
                chord
                    .grace_chords
                    .push(scoretk_core::model::GraceChord::new(
                        scoretk_core::model::GraceType::Acciaccatura,
                        Duration::eighth(),
                    ));
            }
        }
    }

    /// `MeiImporter::addChordRest`: time advance, scaled inside a tuplet.
    fn advance(&self, ticks: &mut Fraction, duration: Duration, _measure: usize) {
        let mut advance = duration.ticks();
        if let Some(ratio) = self.tuplet_ratio {
            advance /= ratio;
        }
        *ticks += advance;
    }

    /// Last chord/rest written on a track, used for attached notations.
    pub(crate) fn last_chord_rest_mut(
        &mut self,
        measure: usize,
        track: Track,
    ) -> Option<&mut ChordRest> {
        let tick = self.score.measures[measure]
            .segments
            .iter()
            .rev()
            .find(|segment| segment.element(track).is_some())?
            .tick;
        let position = self.score.measures[measure]
            .segments
            .iter()
            .position(|segment| segment.tick == tick)?;
        self.score.measures[measure].segments[position]
            .element_mut(track)
            .and_then(SegmentItem::as_chord_rest_mut)
    }

    /// Current time signature as `(numerator, denominator, kind)`.
    pub(crate) fn current_timesig(&self) -> (i64, i64, TimeSigType) {
        let (numerator, denominator, kind) = self
            .timesigs
            .get(&SCOREDEF_IDX)
            .copied()
            .or_else(|| self.timesigs.iter().next().map(|(_, value)| *value))
            .unwrap_or((4, 4, TimeSigType::Normal));
        (i64::from(numerator), i64::from(denominator), kind)
    }
}

/// `Duration` from `@dur` and `@dots`, defaulting to a quarter note
/// (`Convert::durFromMEI` + `AttAugmentDots`).
pub(crate) fn duration_of(node: &Element) -> Duration {
    duration_of_opt(node).unwrap_or(Duration::quarter())
}

fn duration_of_opt(node: &Element) -> Option<Duration> {
    let kind = convert::dur_from_mei(node.attribute("dur")?)?;
    let dots = node
        .attribute("dots")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    Some(Duration::with_dots(kind, dots.min(4)))
}

/// All descendants with the given name, in document order.
pub(crate) fn descendants_named<'a>(node: &'a Element, name: &str) -> Vec<&'a Element> {
    let mut result = Vec::new();
    collect_descendants(node, name, &mut result);
    result
}

fn collect_descendants<'a>(node: &'a Element, name: &str, result: &mut Vec<&'a Element>) {
    for child in &node.children {
        if child.name == name {
            result.push(child);
        }
        collect_descendants(child, name, result);
    }
}
