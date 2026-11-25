// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Keyed per-layer inventory of a [`Score`], and comparison of two inventories.
//!
//! An [`InventoryItem`] is a *comparable item*: a layer, an optional tick, an
//! optional staff, a stable kind and a canonical value. Two inventories are
//! compared as multisets of these keys, so a count alone cannot hide an item
//! that moved or changed value. The comparison is purely in-memory: no I/O,
//! no format knowledge, no clock (AGENTS.md invariant 1).
//!
//! The item kinds are documented in `docs/agents/model-modularisation.md`
//! §11. L0 and L6 have no item kinds yet (their current payloads are either
//! the event spine itself or a zero-sized marker); a layer with no reference
//! item is therefore neutral in the percentages.

use std::collections::BTreeMap;

use super::annotation::Annotation;
use super::chord::ChordRestKind;
use super::fraction::{Fraction, Tick};
use super::layers::Layers;
use super::score::Score;
use super::segment::SegmentItem;
use super::style::StyleSettings;

/// The seven layers in canonical order, for stable reports.
pub const LAYERS: [Layers; 7] = [
    Layers::COMMON,
    Layers::METRIC,
    Layers::EVENTS,
    Layers::TEXT,
    Layers::PLAYBACK,
    Layers::ENGRAVING,
    Layers::EXTENSIONS,
];

/// One comparable item of a layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryItem {
    /// Exactly one layer (not a combination).
    pub layer: Layers,
    /// Exact position in whole-note units, when the item has one.
    pub tick: Option<Tick>,
    /// Staff index, when the item belongs to one.
    pub staff: Option<u32>,
    /// Stable kind, e.g. `"note"`, `"key-sig"`, `"dynamic"`.
    pub kind: &'static str,
    /// Canonical value, e.g. a pitch, a label or a formatted fraction.
    pub value: String,
}

type Key = (u8, Option<Fraction>, Option<u32>, &'static str, String);

impl InventoryItem {
    fn key(&self) -> Key {
        (
            self.layer.bits(),
            self.tick,
            self.staff,
            self.kind,
            self.value.clone(),
        )
    }
}

/// Keyed inventory of the comparable items of one score.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayerInventory {
    /// Items, in traversal order (duplicates are meaningful: two notes at
    /// the same pitch and tick are two attacks).
    pub items: Vec<InventoryItem>,
}

impl LayerInventory {
    /// Empty inventory.
    pub fn new() -> Self {
        Self::default()
    }

    fn push(
        &mut self,
        layer: Layers,
        tick: Option<Tick>,
        staff: Option<u32>,
        kind: &'static str,
        value: impl Into<String>,
    ) {
        self.items.push(InventoryItem {
            layer,
            tick,
            staff,
            kind,
            value: value.into(),
        });
    }

    /// Number of items belonging to `layer` (which may be a set).
    pub fn count(&self, layer: Layers) -> usize {
        self.items
            .iter()
            .filter(|item| layer.contains(item.layer))
            .count()
    }

    /// Items of one kind, across all layers.
    pub fn count_kind(&self, kind: &str) -> usize {
        self.items.iter().filter(|item| item.kind == kind).count()
    }
}

/// Matched / missed / extra items of one layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerCounts {
    /// Comparable items in the reference.
    pub reference: usize,
    /// Reference items found with the same key in the candidate.
    pub matched: usize,
    /// Candidate items with no counterpart in the reference.
    pub extra: usize,
}

impl LayerCounts {
    /// Percentage of reference items matched; 100 % when there is nothing to
    /// compare (an empty layer is not a failure).
    pub fn percent(self) -> f64 {
        if self.reference == 0 {
            100.0
        } else {
            self.matched as f64 * 100.0 / self.reference as f64
        }
    }
}

/// Result of comparing two inventories on a set of declared layers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    /// All declared layers together.
    pub total: LayerCounts,
    /// Counts by single layer, keyed by `Layers::bits()`.
    pub per_layer: BTreeMap<u8, LayerCounts>,
    /// Missing items by `(layer bits, kind)`, for ranking the gaps.
    pub missing_kinds: BTreeMap<(u8, &'static str), usize>,
}

impl Coverage {
    /// Counts of one layer, or zero when that layer has no comparable item.
    pub fn of(&self, layer: Layers) -> LayerCounts {
        self.per_layer
            .get(&layer.bits())
            .copied()
            .unwrap_or_default()
    }
}

fn compare_items(
    reference: &LayerInventory,
    candidate: &LayerInventory,
    layers: Layers,
    kinds: Option<&[&str]>,
) -> Coverage {
    let selected = |item: &InventoryItem| {
        layers.contains(item.layer) && kinds.is_none_or(|ks| ks.contains(&item.kind))
    };
    let expected: Vec<&InventoryItem> = reference
        .items
        .iter()
        .filter(|item| selected(item))
        .collect();
    let actual: Vec<&InventoryItem> = candidate
        .items
        .iter()
        .filter(|item| selected(item))
        .collect();
    let expected =
        expected
            .into_iter()
            .map(InventoryItem::key)
            .fold(BTreeMap::new(), |mut acc, key| {
                *acc.entry(key).or_insert(0) += 1;
                acc
            });
    let mut actual =
        actual
            .into_iter()
            .map(InventoryItem::key)
            .fold(BTreeMap::new(), |mut acc, key| {
                *acc.entry(key).or_insert(0) += 1;
                acc
            });

    let mut coverage = Coverage::default();
    for (key, wanted) in expected {
        let found = actual.get_mut(&key).map_or(0, |count| {
            let taken = (*count).min(wanted);
            *count -= taken;
            taken
        });
        let layer = coverage.per_layer.entry(key.0).or_default();
        layer.reference += wanted;
        layer.matched += found;
        coverage.total.reference += wanted;
        coverage.total.matched += found;
        if wanted > found {
            *coverage.missing_kinds.entry((key.0, key.3)).or_insert(0) += wanted - found;
        }
    }
    for (key, left) in actual {
        coverage.per_layer.entry(key.0).or_default().extra += left;
        coverage.total.extra += left;
    }
    coverage
}

/// Compares two inventories, restricted to the layers `layers` declares.
pub fn compare(reference: &LayerInventory, candidate: &LayerInventory, layers: Layers) -> Coverage {
    compare_items(reference, candidate, layers, None)
}

/// Compares two inventories on a finer format declaration.
///
/// `kinds` restricts the comparison to the item kinds the format can carry
/// (§5.3 of `docs/agents/model-modularisation.md`); `None` keeps the coarse,
/// whole-layer behaviour of [`compare`]. A kind absent from the format is not
/// a loss: it is outside the declaration and is not counted in either ratio.
pub fn compare_with_kinds(
    reference: &LayerInventory,
    candidate: &LayerInventory,
    layers: Layers,
    kinds: Option<&[&str]>,
) -> Coverage {
    compare_items(reference, candidate, layers, kinds)
}

/// "Essential" comparison of today's musical core: `note` and `measure`
/// items only, whatever the declared layers (both are always declared).
pub fn essential(reference: &LayerInventory, candidate: &LayerInventory) -> LayerCounts {
    compare_items(
        reference,
        candidate,
        Layers::ALL,
        Some(&["note", "measure"]),
    )
    .total
}

fn float(value: f64) -> String {
    format!("{value:.6}")
}

/// `StyleSettings` values survive a format round trip with MuseScore's
/// six-significant-digit formatting (`format_style_number` in
/// `scoretk-musicxml-export/src/layout.rs`). Comparing six *decimals* would
/// report a false loss for 0.7874016 → 0.787402, so style values are
/// normalised to five decimals.
fn style_float(value: f64) -> String {
    format!("{value:.5}")
}

/// Builds the comparable inventory of a score.
///
/// Traversal follows the event spine (measures → segments → tracks) so every
/// item carries the tick and staff that a format-independent comparison needs.
pub fn inventory(score: &Score) -> LayerInventory {
    let mut result = LayerInventory::new();

    for measure in &score.measures {
        let start = Some(measure.start_tick);
        result.push(
            Layers::METRIC,
            start,
            None,
            "measure",
            format!("{}:{}", measure.len, measure.nominal_len),
        );
        if measure.start_bar_line != super::segment::BarLineType::Normal {
            result.push(
                Layers::METRIC,
                start,
                None,
                "barline-start",
                format!("{:?}", measure.start_bar_line),
            );
        }
        if measure.end_bar_line != super::segment::BarLineType::Normal {
            result.push(
                Layers::METRIC,
                Some(measure.end_tick()),
                None,
                "barline-end",
                format!("{:?}", measure.end_bar_line),
            );
        }
        if measure.repeat_count > 0 {
            result.push(
                Layers::METRIC,
                Some(measure.end_tick()),
                None,
                "repeat-count",
                measure.repeat_count.to_string(),
            );
        }
        if let Some(count) = measure.multi_measure_rest {
            result.push(
                Layers::METRIC,
                start,
                None,
                "multi-measure-rest",
                count.to_string(),
            );
        }
        if let Some(mark) = measure.measure_repeat {
            result.push(
                Layers::METRIC,
                start,
                Some(mark.staff.raw()),
                "measure-repeat",
                mark.count.to_string(),
            );
        }
        if measure.section_break_before {
            result.push(Layers::METRIC, start, None, "section-break", "1");
        }
        if measure.line_break {
            result.push(
                Layers::ENGRAVING,
                Some(measure.end_tick()),
                None,
                "line-break",
                "1",
            );
        }
        for mark in &measure.repeat_marks {
            match mark {
                super::repeat::RepeatMark::Marker(value) => result.push(
                    Layers::METRIC,
                    start,
                    Some(value.track.staff().raw()),
                    "marker",
                    format!("{:?}:{}", value.kind, value.label),
                ),
                super::repeat::RepeatMark::Jump(value) => result.push(
                    Layers::METRIC,
                    Some(measure.end_tick()),
                    Some(value.track.staff().raw()),
                    "jump",
                    format!("{:?}:{}", value.kind, value.jump_to),
                ),
            }
        }

        for segment in &measure.segments {
            let tick = Some(segment.tick);
            for (track, element) in segment.iter_elements() {
                let staff = Some(track.staff().raw());
                match element {
                    SegmentItem::KeySig(key) => result.push(
                        Layers::METRIC,
                        tick,
                        staff,
                        "key-sig",
                        format!("{}:{:?}", key.concert_key.fifths(), key.mode),
                    ),
                    SegmentItem::TimeSig(time) => result.push(
                        Layers::METRIC,
                        tick,
                        staff,
                        "time-sig",
                        format!("{}:{}:{:?}", time.numerator, time.denominator, time.kind),
                    ),
                    SegmentItem::Clef(clef) => result.push(
                        Layers::METRIC,
                        tick,
                        staff,
                        "clef",
                        format!("{:?}", clef.concert),
                    ),
                    SegmentItem::BarLine(bar) => result.push(
                        Layers::METRIC,
                        tick,
                        staff,
                        "barline",
                        format!("{:?}", bar.kind),
                    ),
                    SegmentItem::Breath(breath) => result.push(
                        Layers::EVENTS,
                        tick,
                        staff,
                        "breath",
                        format!("{:?}", breath.kind),
                    ),
                    SegmentItem::ChordRest(chord_rest) => {
                        let duration = format!(
                            "{:?}:{}",
                            chord_rest.duration.kind, chord_rest.duration.dots
                        );
                        match &chord_rest.kind {
                            ChordRestKind::Rest(rest) => result.push(
                                Layers::EVENTS,
                                tick,
                                staff,
                                "rest",
                                format!("{}:{}", duration, rest.full_measure),
                            ),
                            ChordRestKind::Chord(chord) => {
                                for id in &chord.notes {
                                    let note = &score.notes[id.index()];
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "note",
                                        note.pitch.to_string(),
                                    );
                                    if note.tie_forward.is_some() {
                                        result.push(Layers::EVENTS, tick, staff, "tie", "start");
                                    }
                                    if note.tie_back.is_some() {
                                        result.push(Layers::EVENTS, tick, staff, "tie", "stop");
                                    }
                                    if note.head != super::note::NoteHeadType::Auto
                                        || note.head_group != super::note::NoteHeadGroup::Normal
                                    {
                                        result.push(
                                            Layers::ENGRAVING,
                                            tick,
                                            staff,
                                            "note-head",
                                            format!("{:?}:{:?}", note.head, note.head_group),
                                        );
                                    }
                                    if let Some(velocity) = note.playback.velocity {
                                        result.push(
                                            Layers::PLAYBACK,
                                            tick,
                                            staff,
                                            "velocity",
                                            velocity.to_string(),
                                        );
                                    }
                                    if let Some(user) = note.playback.user_velocity {
                                        result.push(
                                            Layers::PLAYBACK,
                                            tick,
                                            staff,
                                            "user-velocity",
                                            format!("{user}:{:?}", note.playback.velo_type),
                                        );
                                    }
                                }
                                for grace in &chord.grace_chords {
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "grace",
                                        format!("{:?}", grace.chord.grace),
                                    );
                                    for id in &grace.chord.notes {
                                        result.push(
                                            Layers::EVENTS,
                                            tick,
                                            staff,
                                            "grace-note",
                                            score.notes[id.index()].pitch.to_string(),
                                        );
                                    }
                                }
                                for articulation in &chord.articulations {
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "articulation",
                                        format!("{:?}", articulation.kind),
                                    );
                                }
                                for ornament in &chord.ornaments {
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "ornament",
                                        format!("{:?}", ornament.kind),
                                    );
                                }
                                if let Some(tremolo) = chord.tremolo {
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "tremolo",
                                        format!("{:?}", tremolo.kind),
                                    );
                                }
                                if let Some(arpeggio) = chord.arpeggio {
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "arpeggio",
                                        format!("{:?}", arpeggio.kind),
                                    );
                                }
                                if let Some(line) = chord.chord_line {
                                    result.push(
                                        Layers::EVENTS,
                                        tick,
                                        staff,
                                        "chord-line",
                                        format!("{:?}", line),
                                    );
                                }
                                if chord.engraving.stem != super::chord::StemDirection::Auto {
                                    result.push(
                                        Layers::ENGRAVING,
                                        tick,
                                        staff,
                                        "stem",
                                        format!("{:?}", chord.engraving.stem),
                                    );
                                }
                            }
                        }
                        if chord_rest.engraving.beam_mode != super::chord::BeamMode::Auto {
                            result.push(
                                Layers::ENGRAVING,
                                tick,
                                staff,
                                "beam",
                                format!(
                                    "{:?}:{:?}",
                                    chord_rest.engraving.beam_mode,
                                    chord_rest.engraving.beam_direction
                                ),
                            );
                        }
                        if let Some(tuplet) = chord_rest.tuplet {
                            let tuplet = &score.tuplets[tuplet.index()];
                            result.push(
                                Layers::EVENTS,
                                tick,
                                staff,
                                "tuplet-member",
                                format!("{}:{}", tuplet.ratio, tuplet.base_len),
                            );
                        }
                        for lyric in &chord_rest.lyrics {
                            result.push(
                                Layers::TEXT,
                                tick,
                                staff,
                                "lyric",
                                format!("{}:{:?}:{}", lyric.verse, lyric.syllabic, lyric.text),
                            );
                        }
                    }
                }
            }

            for annotation in &segment.annotations {
                let staff = Some(annotation.track.staff().raw());
                let annotation = &annotation.annotation;
                match annotation {
                    Annotation::Dynamic(value) => {
                        result.push(Layers::TEXT, tick, staff, "dynamic", value.text.clone());
                        if let Some(velocity) = value.velocity {
                            result.push(
                                Layers::PLAYBACK,
                                tick,
                                staff,
                                "dynamic-velocity",
                                velocity.to_string(),
                            );
                        }
                    }
                    Annotation::StaffText(value) | Annotation::Expression(value) => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "staff-text",
                        value.text.plain().to_owned(),
                    ),
                    Annotation::SystemText(value) => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "system-text",
                        value.text.plain().to_owned(),
                    ),
                    Annotation::Tempo(value) => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "tempo",
                        format!(
                            "{}:{:?}",
                            value.text.plain(),
                            value.beats_per_second.map(float)
                        ),
                    ),
                    Annotation::RehearsalMark(value) => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "rehearsal-mark",
                        value.text.plain().to_owned(),
                    ),
                    Annotation::InstrumentChange(value) => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "instrument-change",
                        value.text.plain().to_owned(),
                    ),
                    Annotation::Harmony(value) => {
                        result.push(Layers::TEXT, tick, staff, "harmony", value.text.clone())
                    }
                    Annotation::FiguredBass(value) => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "figured-bass",
                        format!("{:?}", value.items),
                    ),
                    Annotation::Fermata(value) => result.push(
                        Layers::EVENTS,
                        tick,
                        staff,
                        "fermata",
                        format!("{:?}", value.kind),
                    ),
                    Annotation::TremoloBar(_) => {
                        result.push(Layers::EVENTS, tick, staff, "tremolo-bar", "1");
                    }
                    Annotation::Other { kind, text, .. } => result.push(
                        Layers::TEXT,
                        tick,
                        staff,
                        "annotation",
                        format!("{kind}:{text}"),
                    ),
                }
            }
        }
    }

    for tuplet in &score.tuplets {
        result.push(
            Layers::METRIC,
            Some(tuplet.tick),
            None,
            "tuplet",
            format!("{}:{}:{}", tuplet.ratio, tuplet.base_len, tuplet.len),
        );
        // The bracket is a MuseScore engraving decision (`calcHasBracket` in
        // `engraving/dom/tuplet.cpp:206-231`), not part of the metric ratio.
        // Only that rendered decision is comparable: `bracket_explicit` says
        // how the file encodes it (`AUTO_BRACKET` vs an explicit
        // `SHOW_BRACKET`/`SHOW_NO_BRACKET`), and the writer may have to make a
        // decision explicit to keep it across a version change. Comparing the
        // encoding would turn that fidelity fix into a false loss.
        result.push(
            Layers::ENGRAVING,
            Some(tuplet.tick),
            None,
            "tuplet-bracket",
            tuplet.bracket.to_string(),
        );
    }

    for spanner in &score.spanners {
        let tick = Some(spanner.start.tick);
        let staff = Some(spanner.start.track.staff().raw());
        let value = format!(
            "{:?}:{:?}:{}",
            spanner.kind, spanner.end.tick, spanner.number
        );
        let kind = match &spanner.kind {
            super::spanner::SpannerType::Slur => "slur",
            super::spanner::SpannerType::Hairpin(_) => "hairpin",
            super::spanner::SpannerType::Ottava(_) => "ottava",
            super::spanner::SpannerType::Pedal => "pedal",
            super::spanner::SpannerType::Volta => "volta",
            super::spanner::SpannerType::TextLine => "text-line",
            super::spanner::SpannerType::Trill => "trill",
            super::spanner::SpannerType::Glissando => "glissando",
            super::spanner::SpannerType::Other(_) => "spanner",
        };
        result.push(
            if kind == "volta" {
                Layers::METRIC
            } else {
                Layers::EVENTS
            },
            tick,
            staff,
            kind,
            value,
        );
    }

    for (name, value) in [
        ("work_title", &score.metadata.work_title),
        ("movement_title", &score.metadata.movement_title),
        ("movement_number", &score.metadata.movement_number),
        ("work_number", &score.metadata.work_number),
        ("composer", &score.metadata.composer),
        ("lyricist", &score.metadata.lyricist),
        ("poet", &score.metadata.poet),
        ("translator", &score.metadata.translator),
        ("arranger", &score.metadata.arranger),
        ("copyright", &score.metadata.copyright),
        ("source", &score.metadata.source),
        ("subtitle", &score.metadata.subtitle),
    ] {
        if !value.is_empty() {
            result.push(Layers::TEXT, None, None, "title", format!("{name}:{value}"));
        }
    }
    for (index, credit) in score.engraving.credits.iter().enumerate() {
        result.push(
            Layers::TEXT,
            None,
            None,
            "credit",
            format!("{}:{}", index, credit.text.plain()),
        );
        result.push(
            Layers::ENGRAVING,
            None,
            None,
            "credit-position",
            format!(
                "{}:{:?}:{:?}:{}:{}:{}",
                index,
                credit.justify,
                credit.valign,
                credit.page,
                credit.font_size.map(style_float).unwrap_or_default(),
                style_float(credit.vbox_height)
            ),
        );
    }
    for (index, text) in score.engraving.tbox_texts.iter().enumerate() {
        result.push(
            Layers::TEXT,
            None,
            None,
            "tbox-text",
            format!("{}:{}", index, text.text.plain()),
        );
        result.push(
            Layers::ENGRAVING,
            None,
            None,
            "tbox-position",
            format!(
                "{}:{}:{:?}:{}",
                index,
                text.measure,
                text.placement,
                text.font_size.map(style_float).unwrap_or_default()
            ),
        );
    }

    for part in &score.parts {
        let instrument = &part.instrument;
        result.push(
            Layers::PLAYBACK,
            None,
            None,
            "instrument",
            format!("{}:{}", part.id.raw(), instrument.id),
        );
        if let Some(program) = instrument.midi_program {
            result.push(Layers::PLAYBACK, None, None, "program", program.to_string());
        }
        if let Some(channel) = instrument.midi_channel {
            result.push(Layers::PLAYBACK, None, None, "channel", channel.to_string());
        }
        if let Some(volume) = instrument.volume {
            result.push(Layers::PLAYBACK, None, None, "volume", volume.to_string());
        }
        if let Some(pan) = instrument.pan {
            result.push(Layers::PLAYBACK, None, None, "pan", pan.to_string());
        }
        for articulation in &instrument.midi_articulations {
            // The `<Instrument><Articulation>` table is a MuseScore instrument
            // definition (`engraving/dom/instrument.h::MidiArticulation`), not a
            // format-level playback item: MusicXML's `<midi-instrument>` only
            // carries channel/program/volume/pan (format-ref/musicxml.md §"Clef,
            // time, staves, parts"), SMF has channel voice messages and meta
            // events but no articulation table (format-ref/midi.md §"Events and
            // time", §"Instruments, parts, repeats"), ABC's `%%MIDI` directives
            // cannot reconstruct one (format-mapping/abc.md §6.2 and "Model
            // gaps" n°5) and LilyPond has no equivalent. Its *effect* can be
            // rendered as note velocity, but the table itself is an L6 payload
            // that only MuseScore carries.
            result.push(
                Layers::EXTENSIONS,
                None,
                None,
                "midi-articulation",
                format!(
                    "{}:{}:{}",
                    articulation.name, articulation.velocity, articulation.gate_time
                ),
            );
        }
    }
    if score.playback.articulate {
        result.push(Layers::PLAYBACK, None, None, "articulate", "1");
    }
    if let Some(transposition) = score.playback.transposition {
        result.push(
            Layers::PLAYBACK,
            None,
            None,
            "playback-transposition",
            format!("{:?}", transposition),
        );
    }

    // Page geometry, scaling and credit font sizes are real format data:
    // MusicXML has `<defaults><scaling>/<page-layout>/<page-margins>` and
    // `<credit-words font-size>` (format-ref/musicxml.md §"Document shape"),
    // LilyPond has `\paper` (format-mapping/lilypond.md, `\paper` row). The
    // other fields below are MuseScore's own layout defaults: no format spec
    // carries `hairpinHeight`, `smallStaffMag`, `dontHideStavesInFirstSystem`,
    // `concertPitch`, `createMultiMeasureRests`, `pageCount` or
    // `minEmptyMeasures`. They are L6 extensions, not L5 format items.
    let style_item = |result: &mut LayerInventory, name: &str, value: String| {
        let internal = matches!(
            name,
            "hairpin_height_sp"
                | "small_staff_mag"
                | "dont_hide_staves_in_first_system"
                | "concert_pitch"
                | "create_multi_measure_rests"
                | "page_count"
                | "min_empty_measures"
        );
        result.push(
            if internal {
                Layers::EXTENSIONS
            } else {
                Layers::ENGRAVING
            },
            None,
            None,
            if internal { "style-internal" } else { "style" },
            value,
        );
    };
    let style = &score.engraving.style;
    let default = StyleSettings::default();
    for (name, value, baseline) in [
        ("spatium_mm", style.spatium_mm, default.spatium_mm),
        ("page_width_in", style.page_width_in, default.page_width_in),
        (
            "page_height_in",
            style.page_height_in,
            default.page_height_in,
        ),
        (
            "page_even_left_margin_in",
            style.page_even_left_margin_in,
            default.page_even_left_margin_in,
        ),
        (
            "page_odd_left_margin_in",
            style.page_odd_left_margin_in,
            default.page_odd_left_margin_in,
        ),
        (
            "page_even_right_margin_in",
            style.page_even_right_margin_in,
            default.page_even_right_margin_in,
        ),
        (
            "page_odd_right_margin_in",
            style.page_odd_right_margin_in,
            default.page_odd_right_margin_in,
        ),
        (
            "page_even_top_margin_in",
            style.page_even_top_margin_in,
            default.page_even_top_margin_in,
        ),
        (
            "page_odd_top_margin_in",
            style.page_odd_top_margin_in,
            default.page_odd_top_margin_in,
        ),
        (
            "page_even_bottom_margin_in",
            style.page_even_bottom_margin_in,
            default.page_even_bottom_margin_in,
        ),
        (
            "page_odd_bottom_margin_in",
            style.page_odd_bottom_margin_in,
            default.page_odd_bottom_margin_in,
        ),
        (
            "title_font_size",
            style.title_font_size,
            default.title_font_size,
        ),
        (
            "subtitle_font_size",
            style.subtitle_font_size,
            default.subtitle_font_size,
        ),
        (
            "composer_font_size",
            style.composer_font_size,
            default.composer_font_size,
        ),
        (
            "lyricist_font_size",
            style.lyricist_font_size,
            default.lyricist_font_size,
        ),
        (
            "frame_font_size",
            style.frame_font_size,
            default.frame_font_size,
        ),
        (
            "instrument_excerpt_font_size",
            style.instrument_excerpt_font_size,
            default.instrument_excerpt_font_size,
        ),
        (
            "footer_font_size",
            style.footer_font_size,
            default.footer_font_size,
        ),
        (
            "hairpin_height_sp",
            style.hairpin_height_sp,
            default.hairpin_height_sp,
        ),
        (
            "small_staff_mag",
            style.small_staff_mag,
            default.small_staff_mag,
        ),
    ] {
        if (value - baseline).abs() > 1e-9 {
            style_item(&mut result, name, format!("{name}:{}", style_float(value)));
        }
    }
    for (name, value, baseline) in [
        (
            "page_two_sided",
            style.page_two_sided,
            default.page_two_sided,
        ),
        (
            "hide_empty_staves",
            style.hide_empty_staves,
            default.hide_empty_staves,
        ),
        (
            "dont_hide_staves_in_first_system",
            style.dont_hide_staves_in_first_system,
            default.dont_hide_staves_in_first_system,
        ),
        ("concert_pitch", style.concert_pitch, default.concert_pitch),
        (
            "create_multi_measure_rests",
            style.create_multi_measure_rests,
            default.create_multi_measure_rests,
        ),
    ] {
        if value != baseline {
            style_item(&mut result, name, format!("{name}:{value}"));
        }
    }
    if style.lyrics_placement != default.lyrics_placement {
        style_item(
            &mut result,
            "lyrics_placement",
            format!("lyrics_placement:{:?}", style.lyrics_placement),
        );
    }
    if style.page_count != default.page_count {
        style_item(
            &mut result,
            "page_count",
            format!("page_count:{}", style.page_count),
        );
    }
    if style.min_empty_measures != default.min_empty_measures {
        style_item(
            &mut result,
            "min_empty_measures",
            format!("min_empty_measures:{}", style.min_empty_measures),
        );
    }

    // L6: the GABC syllable/neume stream is primary data, not a projection.
    // `Gabc.tex:67-154` (headers), `:228-343` (syllables), `:408-443`
    // (pitch letters), `:702-845` (signs and separation bars) and `:847-870`
    // (clefs). The derived modern view in L0/L1 is inventoried as ordinary
    // notes/measures; the `derived_modern_view` flag records its status.
    if let Some(super::layers::Extension::Gregorian(gregorian)) = &score.extensions.format {
        for header in &gregorian.headers {
            result.push(
                Layers::EXTENSIONS,
                None,
                None,
                "gregorian-header",
                format!("{}:{}", header.name, header.value),
            );
        }
        for (index, clef) in gregorian.clefs.iter().enumerate() {
            result.push(
                Layers::EXTENSIONS,
                None,
                None,
                "gregorian-clef",
                format!("{index}:{clef}"),
            );
        }
        for (index, syllable) in gregorian.syllables.iter().enumerate() {
            result.push(
                Layers::EXTENSIONS,
                None,
                None,
                "gregorian-syllable",
                format!("{index}:{}", syllable.text),
            );
            if let Some(rubric) = &syllable.rubric {
                result.push(
                    Layers::EXTENSIONS,
                    None,
                    None,
                    "gregorian-rubric",
                    format!("{index}:{rubric}"),
                );
            }
            for note in &syllable.notes {
                result.push(
                    Layers::EXTENSIONS,
                    None,
                    None,
                    "gregorian-note",
                    format!("{}:{}", note.letter, note.glyph),
                );
                for sign in &note.signs {
                    result.push(
                        Layers::EXTENSIONS,
                        None,
                        None,
                        "gregorian-sign",
                        sign.raw.clone(),
                    );
                }
            }
            for sign in &syllable.signs {
                result.push(
                    Layers::EXTENSIONS,
                    None,
                    None,
                    "gregorian-sign",
                    sign.raw.clone(),
                );
            }
            for bar in &syllable.bars {
                result.push(
                    Layers::EXTENSIONS,
                    None,
                    None,
                    "gregorian-bar",
                    bar.as_str(),
                );
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, Score};

    fn score_with_notes() -> Score {
        let mut score = Score::new();
        let part = score.add_part(crate::model::Part::new("Piano", "Pno"));
        let staff = score.add_staff(part);
        let mut measure = crate::model::Measure::new(0, Fraction::ZERO, Fraction::ONE);
        let mut segment =
            crate::model::Segment::new(crate::model::SegmentType::ChordRest, Fraction::ZERO, 1);
        let track = crate::model::Track::new(staff, 0);
        let note = score.add_note(Note::new(60, 14));
        let mut chord_rest = crate::model::ChordRest::chord(crate::model::Duration::whole());
        chord_rest.kind = ChordRestKind::Chord(crate::model::Chord {
            notes: vec![note],
            ..crate::model::Chord::default()
        });
        segment.set_element(track, crate::model::SegmentItem::ChordRest(chord_rest));
        measure.add_segment(segment);
        score.add_measure(measure);
        score
    }

    #[test]
    fn inventory_is_keyed_by_tick_staff_kind_and_value() {
        let inventory = inventory(&score_with_notes());
        assert_eq!(inventory.count_kind("note"), 1);
        assert_eq!(inventory.count_kind("measure"), 1);
        let note = inventory
            .items
            .iter()
            .find(|item| item.kind == "note")
            .expect("one note");
        assert_eq!(note.tick, Some(Fraction::ZERO));
        assert_eq!(note.staff, Some(0));
        assert_eq!(note.value, "60");
        assert_eq!(note.layer, Layers::EVENTS);
    }

    #[test]
    fn midi_articulation_table_is_a_musescore_extension() {
        let mut score = Score::new();
        let part = score.add_part(crate::model::Part::new("Piano", "Pno"));
        score.parts[part.index()]
            .instrument
            .midi_articulations
            .push(crate::model::MidiArticulation {
                name: "staccato".to_owned(),
                velocity: 80,
                gate_time: 50,
            });
        let found = inventory(&score)
            .items
            .into_iter()
            .find(|item| item.kind == "midi-articulation")
            .expect("the articulation table is inventoried");
        assert_eq!(found.layer, Layers::EXTENSIONS);
        let own = LayerInventory { items: vec![found] };
        assert_eq!(compare(&own, &own, Layers::PLAYBACK).total.reference, 0);
        assert_eq!(compare(&own, &own, Layers::ALL).total.reference, 1);
    }

    #[test]
    fn muse_score_only_style_defaults_are_extensions() {
        let mut score = Score::new();
        score.engraving.style.min_empty_measures += 1;
        score.engraving.style.spatium_mm += 1.0;
        let items = inventory(&score).items;
        let internal = items
            .iter()
            .find(|item| item.kind == "style-internal")
            .expect("the MuseScore-only default is inventoried");
        assert_eq!(internal.layer, Layers::EXTENSIONS);
        assert!(internal.value.starts_with("min_empty_measures:"));
        let page = items
            .iter()
            .find(|item| item.kind == "style")
            .expect("page geometry stays a format item");
        assert_eq!(page.layer, Layers::ENGRAVING);
        assert!(page.value.starts_with("spatium_mm:"));
    }

    #[test]
    fn style_values_are_normalised_to_musescore_precision() {
        let mut score = Score::new();
        score.engraving.style.page_odd_bottom_margin_in = 0.7874016;
        let found = inventory(&score)
            .items
            .into_iter()
            .find(|item| item.kind == "style")
            .expect("a style item");
        assert_eq!(found.value, "page_odd_bottom_margin_in:0.78740");
    }

    #[test]
    fn comparison_matches_items_not_only_counts() {
        let mut reference = LayerInventory::new();
        reference.push(Layers::EVENTS, Some(Fraction::ZERO), Some(0), "note", "60");
        reference.push(Layers::EVENTS, Some(Fraction::ZERO), Some(0), "note", "62");
        let mut candidate = LayerInventory::new();
        candidate.push(Layers::EVENTS, Some(Fraction::ZERO), Some(0), "note", "62");
        candidate.push(Layers::EVENTS, Some(Fraction::ZERO), Some(1), "note", "60");
        let coverage = compare(&reference, &candidate, Layers::EVENTS);
        assert_eq!(coverage.total.reference, 2);
        assert_eq!(coverage.total.matched, 1);
        assert_eq!(coverage.total.extra, 1);
        assert_eq!(
            coverage.missing_kinds.get(&(Layers::EVENTS.bits(), "note")),
            Some(&1)
        );
        assert_eq!(coverage.of(Layers::EVENTS).percent(), 50.0);
    }

    #[test]
    fn kind_declaration_excludes_kinds_the_format_cannot_carry() {
        let mut reference = LayerInventory::new();
        reference.push(Layers::METRIC, None, None, "measure", "1/1:1/1");
        reference.push(Layers::METRIC, None, None, "key-sig", "0:Unknown");
        let own = reference.clone();
        let smf = compare_with_kinds(&reference, &own, Layers::METRIC, Some(&["key-sig"]));
        assert_eq!(smf.total.reference, 1);
        assert_eq!(smf.total.matched, 1);
        assert_eq!(compare(&reference, &own, Layers::METRIC).total.reference, 2);
    }

    #[test]
    fn essential_counts_only_notes_and_measures() {
        let mut reference = LayerInventory::new();
        reference.push(Layers::EVENTS, None, None, "note", "60");
        reference.push(Layers::METRIC, None, None, "measure", "1/1:1/1");
        reference.push(Layers::TEXT, None, None, "lyric", "ha");
        let counts = essential(&reference, &reference);
        assert_eq!(counts.reference, 2);
        assert_eq!(counts.matched, 2);
        assert_eq!(counts.percent(), 100.0);
    }

    #[test]
    fn gregorian_payload_is_a_declared_l6_item_set() {
        use super::super::gregorian::{
            BarKind, Gregorian, GregorianHeader, GregorianNote, NeumeSign, SignKind, Syllable,
        };
        use super::super::layers::Extension;

        fn score_with_chant() -> Score {
            let mut score = Score::new();
            score.extensions.format = Some(Extension::Gregorian(Gregorian {
                headers: vec![GregorianHeader {
                    name: "name".to_owned(),
                    value: "Veni".to_owned(),
                }],
                staff_lines: 4,
                clefs: vec!["c3".to_owned()],
                syllables: vec![Syllable {
                    text: "Ve(g)ni(h)".to_owned(),
                    clef: None,
                    rubric: None,
                    notes: vec![GregorianNote {
                        letter: 'g',
                        glyph: String::new(),
                        signs: vec![NeumeSign::new(SignKind::PunctumMora, ".")],
                    }],
                    signs: Vec::new(),
                    bars: vec![BarKind::DivisioFinalis],
                    space_after: false,
                }],
                derived_modern_view: true,
            }));
            score
        }

        let reference = inventory(&score_with_chant());
        for kind in [
            "gregorian-header",
            "gregorian-clef",
            "gregorian-syllable",
            "gregorian-note",
            "gregorian-sign",
            "gregorian-bar",
        ] {
            assert_eq!(reference.count_kind(kind), 1, "{kind}");
        }
        assert_eq!(reference.items[0].layer, Layers::EXTENSIONS);
        let empty = LayerInventory::new();
        assert_eq!(
            compare(&reference, &empty, Layers::EXTENSIONS)
                .total
                .reference,
            6
        );
        // L0/L1-only directions do not see the payload.
        assert_eq!(
            compare(&reference, &empty, Layers::METRIC).total.reference,
            0
        );
    }

    #[test]
    fn undeclared_layers_are_ignored() {
        let mut reference = LayerInventory::new();
        reference.push(Layers::TEXT, None, None, "lyric", "ha");
        reference.push(Layers::EVENTS, None, None, "note", "60");
        let coverage = compare(&reference, &reference, Layers::EVENTS);
        assert_eq!(coverage.total.reference, 1);
        assert_eq!(coverage.of(Layers::TEXT).reference, 0);
    }
}
