// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/systemlayout.cpp:2900-2991`
// (`minDistance`), `pagelayout.cpp:524-613` (`layoutPage`) and
// `chordlayout.cpp:1287-1442` (`updateLedgerLines`), `:2468-2564`
// (`placeDots`) of MuseScore Studio 4.7.4 (tag v4.7.4), Copyright (C)
// MuseScore Limited and others, modified (rewritten in Rust) in 2026.
// The `EngravingItem` graph is replaced by system/page geometry from atoms
// plus plain vertical intervals.
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

//! Page-local placement from precomputed atoms. `page(n)` never touches the
//! `Score` and never recomputes an atom.

use alloc::vec;
use alloc::vec::Vec;

use scoretk_core::model::{
    BeamMode, Credit, CreditJustify, CreditStyle, LyricsSyllabic, Score, StemDirection,
};
use scoretk_smufl::{anchor, glyph, text_advance, text_glyph, units_to_fixed, UPEM};

use crate::atoms::{flag_name, rest_name, Atom, AtomKind, ChordAtom, LyricAtom, MeasureAtom};
use crate::display::{DisplayItem, DisplayList, Geometry, ItemKind};
use crate::style::*;

/// Page geometry in fixed 1/1024 staff-space units.
///
/// // Port of `styledef.cpp:41-59, 279` (4.7.4 defaults): A4 210 × 297 mm,
/// 15 mm margins, 180 mm printable width, `minSystemDistance` 8.5 sp,
/// `maxSystemDistance` 15 sp and `lastSystemFillLimit` 0.3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSpec {
    /// Printable width (180 mm on A4 with 15 mm margins at 1.75 mm/sp).
    pub printable_width: i32,
    /// Left margin from the page edge.
    pub margin_left: i32,
    /// Top margin from the page edge.
    pub margin_top: i32,
    /// A4 page height (297 mm / 1.75 mm per sp).
    pub page_height: i32,
    /// // Port of `styledef.cpp:2931` (`minSystemDistance` 8.5 sp).
    pub min_system_distance: i32,
    /// // Port of `styledef.cpp:2933` (`maxSystemDistance` 15 sp).
    pub max_system_distance: i32,
    /// // Port of `styledef.cpp:279` (`lastSystemFillLimit` 0.3).
    pub last_system_fill_limit: i32,
    /// // Port of `styledef.cpp:540` (`firstSystemIndentationValue` 5 sp):
    /// the left indent of the first system when the score opens with a
    /// vertical frame (title, subtitle, composer).
    pub first_system_indent: i32,
    /// Spatium in millimetres, to convert point font sizes into staff spaces.
    pub spatium_mm: f64,
    /// Height of the first-page VBox plus `frameSystemDistance` (7 sp),
    /// added above the first system. Zero when the score has no title frame.
    pub title_frame_height: i32,
    /// // Port of `styledef.cpp:55` (`staffDistance` 6.5 sp): gap between
    /// staves of different parts.
    pub staff_distance: i32,
    /// // Port of `styledef.cpp:57` (`akkoladeDistance` 6.5 sp): gap between
    /// staves of the same part (grand staff).
    pub akkolade_distance: i32,
    /// Stretch factors and bounds of the vertical spread pass
    /// (`styledef.cpp:62-70`, `verticalgapdata.cpp:71-128`).
    pub enable_vertical_spread: bool,
    pub spread_system: i32,
    pub spread_square_bracket: i32,
    pub spread_curly_bracket: i32,
    pub max_page_fill_spread: i32,
    pub min_staff_spread: i32,
    pub max_staff_spread: i32,
    pub max_system_spread: i32,
    pub max_akkolade_distance: i32,
    pub staff_upper_border: i32,
    pub staff_lower_border: i32,
    pub frame_system_distance: i32,
    pub instrument_name_offset: i32,
    /// Point size of the lyricist text style (`styledef.cpp:878`, default
    /// 10 pt). // Port of `LyricsLayout::layout`.
    pub lyric_font_size: f64,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self {
            printable_width: 103 * SP,
            margin_left: 9 * SP,
            margin_top: 9 * SP,
            page_height: 170 * SP,
            min_system_distance: 8704, // styledef.cpp:58 (8.5 sp)
            max_system_distance: 15 * SP,
            last_system_fill_limit: 307,
            first_system_indent: 0,
            spatium_mm: 1.75,
            title_frame_height: 0,
            staff_distance: 6656,
            akkolade_distance: 6656,
            enable_vertical_spread: true,
            spread_system: 2560,
            spread_square_bracket: 1229,
            spread_curly_bracket: 1126,
            max_page_fill_spread: 6144,
            min_staff_spread: 3584,
            max_staff_spread: 20 * SP,
            max_system_spread: 32 * SP,
            max_akkolade_distance: 6656,
            staff_upper_border: 7 * SP,
            staff_lower_border: 7 * SP,
            frame_system_distance: 7 * SP,
            instrument_name_offset: SP,
            lyric_font_size: 10.0,
        }
    }
}

impl PageSpec {
    /// Derives page geometry from the score's `<Style>` (`pagestyle.cpp:26-41`,
    /// `styledef.cpp:41-59`). A 3.x score with 10 mm margins and a 1.764 mm
    /// spatium has a 107.7 sp printable width, not the 4.x default 103 sp;
    /// using the default changed every measure width and every line break.
    /// Arithmetic stays in `f64` here (no libm call), then rounds to fixed
    /// staff spaces; the rest of the engraver stays fixed point.
    pub fn from_score(score: &Score) -> Self {
        let style = &score.engraving.style;
        let spatium = style.spatium_mm.max(0.1);
        let to_fixed = |mm: f64| ((mm / spatium) * SP as f64 + 0.5) as i32;
        let left = style.page_odd_left_margin_in * 25.4;
        let top = style.page_odd_top_margin_in * 25.4;
        Self {
            printable_width: to_fixed(style.page_printable_width_in * 25.4),
            margin_left: to_fixed(left),
            margin_top: to_fixed(top),
            page_height: to_fixed(style.page_height_in * 25.4),
            last_system_fill_limit: (style.last_system_fill_limit * 1024.0) as i32,
            // `enableIndentationOnFirstSystem` is true by default in 4.x
            // (legacy 3.x style files disable it).
            first_system_indent: if style.enable_indentation_on_first_system {
                (style.first_system_indentation_sp * SP as f64) as i32
            } else {
                0
            },
            spatium_mm: spatium,
            staff_distance: (style.staff_distance_sp * SP as f64) as i32,
            akkolade_distance: (style.akkolade_distance_sp * SP as f64) as i32,
            min_system_distance: (style.min_system_distance_sp * SP as f64) as i32,
            max_system_distance: (style.max_system_distance_sp * SP as f64) as i32,
            enable_vertical_spread: style.enable_vertical_spread,
            spread_system: (style.spread_system * SP as f64) as i32,
            spread_square_bracket: (style.spread_square_bracket * SP as f64) as i32,
            spread_curly_bracket: (style.spread_curly_bracket * SP as f64) as i32,
            max_page_fill_spread: (style.max_page_fill_spread_sp * SP as f64) as i32,
            min_staff_spread: (style.min_staff_spread_sp * SP as f64) as i32,
            max_staff_spread: (style.max_staff_spread_sp * SP as f64) as i32,
            max_system_spread: (style.max_system_spread_sp * SP as f64) as i32,
            max_akkolade_distance: (style.max_akkolade_distance_sp * SP as f64) as i32,
            staff_upper_border: (style.staff_upper_border_sp * SP as f64) as i32,
            staff_lower_border: (style.staff_lower_border_sp * SP as f64) as i32,
            frame_system_distance: (style.frame_system_distance_sp * SP as f64) as i32,
            instrument_name_offset: (style.instrument_name_offset_sp * SP as f64) as i32,
            lyric_font_size: style.lyricist_font_size,
            // // Port of `boxlayout.cpp` and `systemlayout.cpp`: a VBox on
            // page 1 pushes the first system down by its own height plus
            // `frameSystemDistance` (7 sp, styledef.cpp:139).
            title_frame_height: score
                .engraving
                .credits
                .iter()
                .filter(|credit| credit.page == 1)
                .map(|credit| (credit.vbox_height * SP as f64) as i32)
                .max()
                .map_or(0, |height| {
                    height + (style.frame_system_distance_sp * SP as f64) as i32
                }),
        }
    }

    /// Converts a fixed staff-space coordinate to millimetres (spatium 1.75 mm).
    pub fn fixed_to_mm(fixed: i32) -> f64 {
        (fixed as f64) * 1.75 / SP as f64
    }

    /// Page width in millimetres.
    pub fn page_width_mm(&self) -> f64 {
        Self::fixed_to_mm(self.printable_width + 2 * self.margin_left)
    }

    /// Page height in millimetres.
    pub fn page_height_mm(&self) -> f64 {
        Self::fixed_to_mm(self.page_height)
    }
}

/// Beam width (0.5 sp, `styledef.cpp:225`) and distance between successive
/// beams (`beamSpacing = 3`, `beamlayout.cpp:328-331`: 3/4 sp).
const BEAM_WIDTH: i32 = 512;
const BEAM_DIST: i32 = 768;
/// // Port of `beamtremololayout.cpp:48` (`_maxSlopes`): the maximum slant
/// in quarter spaces for a given note interval (seconds, thirds, …).
const MAX_SLOPES: [i32; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

/// Beam geometry of one system: display items plus a stem override per
/// event (system-local event index).
#[derive(Default)]
struct BeamPlan {
    items: Vec<DisplayItem>,
    /// `Some((stem_up, stem_length))` for a chord whose stem must reach a
    /// beam instead of stopping at the default length.
    stems: Vec<Option<(bool, i32)>>,
}

fn round_quarter(value: i32) -> i32 {
    // Rust truncates toward zero; round half away from zero like C++'s
    // `std::round` (`beamtremololayout.cpp:703-704`).
    if value >= 0 {
        (value + 128) / 256 * 256
    } else {
        (value - 128) / 256 * 256
    }
}

fn beam_no_continue(mode: BeamMode) -> bool {
    matches!(mode, BeamMode::End | BeamMode::None)
}

fn beam_mode_mid(mode: BeamMode) -> bool {
    matches!(mode, BeamMode::Mid | BeamMode::Begin16 | BeamMode::Begin32)
}

/// // Port of `BeamLayout::createBeams` (`beamlayout.cpp:622-809`) and
/// `beamGraceNotes` (`:546-619`): walk the events of one voice and split
/// them at beam boundaries. `beamNoContinue` ends a group, `beamModeMid`
/// continues it, `Begin` starts a new one, `Auto` continues over rests and
/// short notes only while the strokes continue.
fn beam_groups(events: &[(i32, &Atom)], voice: u32) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (index, (_, event)) in events.iter().enumerate() {
        if event.voice != voice {
            continue;
        }
        let mode = event.beam_mode;
        let close = |current: &mut Vec<usize>, groups: &mut Vec<Vec<usize>>| {
            if current.len() > 1 {
                groups.push(core::mem::take(current));
            } else {
                current.clear();
            }
        };
        if beam_no_continue(mode) {
            close(&mut current, &mut groups);
            continue;
        }
        if event.beam_start
            || matches!(
                mode,
                BeamMode::Begin | BeamMode::Begin16 | BeamMode::Begin32
            )
        {
            close(&mut current, &mut groups);
            current.push(index);
            continue;
        }
        // `beamModeMid` keeps a beam open over a segment or measure
        // boundary (`beamlayout.cpp:660, 748`); an `Auto` event only
        // continues while it carries strokes or is a rest beamed over, and
        // never crosses a measure without an explicit continuation.
        let crosses_measure = current
            .last()
            .is_some_and(|&previous| events[previous].1.measure != event.measure);
        if crosses_measure && !beam_mode_mid(mode) {
            close(&mut current, &mut groups);
        }
        if event.beams == 0 && !matches!(event.kind, AtomKind::Rest { .. }) && !beam_mode_mid(mode)
        {
            close(&mut current, &mut groups);
            continue;
        }
        current.push(index);
    }
    if current.len() > 1 {
        groups.push(current);
    }
    groups
}

/// // Port of `BeamTremoloLayout::calculateAnchors`
/// (`beamtremololayout.cpp:591-770`) and `computeDesiredSlant` (`:1094-1156`):
/// the beam's two ends sit on the default stem tips of the first and last
/// chords, quantised to quarter spaces; the slant is limited both by the
/// note interval and by the beam's width in staff spaces. The inner stems
/// are then extended to the beam line (`beamlayout.cpp:1262-1307`).
fn plan_beams(events: &[(i32, &Atom)], multi_voice: bool, system_y: i32) -> BeamPlan {
    let mut plan = BeamPlan {
        items: Vec::new(),
        stems: vec![None; events.len()],
    };
    let mut voices = Vec::new();
    for (_, event) in events {
        if !voices.contains(&event.voice) {
            voices.push(event.voice);
        }
    }
    for voice in voices {
        for group in beam_groups(events, voice) {
            if group.iter().all(|&index| events[index].1.beams == 0) {
                continue;
            }
            // Automatically grouped tuplets/beams still use the default
            // stems here: their slant is governed by the tuplet beam
            // grouping of `Groups::actualBeamMode`, which this page-local
            // pass does not yet reproduce. Only groups anchored by an
            // explicit `<Beam>` or `BeamMode` are ported (P3 debt).
            let anchored = group.first().is_some_and(|&index| {
                events[index].1.beam_start || !matches!(events[index].1.beam_mode, BeamMode::Auto)
            });
            if !anchored {
                continue;
            }
            let chords: Vec<usize> = group
                .iter()
                .copied()
                .filter(|&index| events[index].1.chord.is_some())
                .collect();
            if chords.len() < 2 {
                continue;
            }
            let first = chords[0];
            let last = chords[chords.len() - 1];
            let explicit = events[group[0]].1.beam_dir;
            // // Port of `Beam::layout1` (`beamlayout.cpp:169-199`) and
            // `Chord::stemDirection` (`chordlayout.cpp:1584-1588`): the group
            // follows the first chord's stem direction, overridden by an
            // explicit `<Beam><StemDirection>`. That chord already carries
            // the tick-overlap voice rule of `Measure::hasVoices`
            // (`measure.cpp:2360-2419`); the former system-wide voice count
            // forced odd voices down even when they never overlapped, so
            // the beam was anchored above notes whose stems were down and
            // `far_end - note_y` collapsed to zero (no visible stem).
            let first_chord = events[first].1.chord.as_ref();
            let up = match explicit {
                Some(StemDirection::Up) => true,
                Some(StemDirection::Down) => false,
                _ => first_chord.map_or_else(
                    || {
                        if multi_voice {
                            voice.is_multiple_of(2)
                        } else {
                            let lines: Vec<i32> = group
                                .iter()
                                .filter_map(|&index| events[index].1.chord.as_ref())
                                .flat_map(|chord| chord.notes.iter().map(|note| note.line))
                                .collect();
                            crate::atoms::auto_stem_up(&lines)
                        }
                    },
                    |chord| chord.stem_up,
                ),
            };
            // Override every chord of the group with the beam direction
            // (`beamlayout.cpp:169-199`, `layout1`).
            for &index in &chords {
                if let Some(chord) = &events[index].1.chord {
                    plan.stems[index] = Some((up, chord.stem_len));
                }
            }
            // // Port of `beamtremololayout.cpp:1368-1375`: the anchor's note
            // is `downNote()` for an up beam and `upNote()` for a down beam,
            // i.e. precisely the line `place_chord` starts the stem from.
            // The old `min`/`max` pairing shifted the far end of a multi-note
            // chord by the height of the chord.
            let anchor_line = |index: usize| {
                let chord = events[index].1.chord.as_ref().unwrap();
                let lines = chord.notes.iter().map(|note| note.line);
                if up {
                    lines.max().unwrap_or(4)
                } else {
                    lines.min().unwrap_or(4)
                }
            };
            let start_line = anchor_line(first);
            let end_line = anchor_line(last);
            // The anchor is the stem tip in the group's direction, before
            // the beam is attached (quarter-space grid). Using the chord's
            // own direction here made an explicit down beam anchor above
            // its notes: `Beam::layout1` applies one direction to the whole
            // group (`beamlayout.cpp:169-199`).
            let anchor = |index: usize| {
                let chord = events[index].1.chord.as_ref().unwrap();
                let note_y = anchor_line(index) * HALF_SP;
                let tip = if up {
                    note_y - chord.stem_len
                } else {
                    note_y + chord.stem_len
                };
                round_quarter(tip)
            };
            let start_y = anchor(first);
            let end_y = anchor(last);
            // // Port of `chordBeamAnchorX` (`beamtremololayout.cpp:1293-1328`):
            // a beam runs from the stem of its first chord to the stem of its
            // last one, not from their notehead origins. The old notehead x
            // left both ends one notehead width (~1.25 sp) short of the stems.
            let stem_x = |index: usize| {
                // A beam group may contain a rest (an explicit `<Beam>` can
                // span one). Rests have no stem or notehead anchor; the slab
                // still starts at their segment x instead of panicking.
                let Some(chord) = events[index].1.chord.as_ref() else {
                    return events[index].0;
                };
                let head = chord
                    .notes
                    .first()
                    .map_or("noteheadBlack", |note| note.head);
                let head_w = glyph(head).map(|g| units_to_fixed(g.bbox[2])).unwrap_or(SP);
                if up {
                    events[index].0 + head_w - STEM_WIDTH / 2
                } else {
                    events[index].0 - STEM_WIDTH / 2
                }
            };
            let x1 = stem_x(first);
            let x2 = stem_x(last);
            if x2 <= x1 {
                continue;
            }
            let width_sp = (x2 - x1) / SP;
            // // Port of `getMaxSlope` (`beamtremololayout.cpp:1252-1271`).
            let max_slope = if width_sp < 3 {
                1
            } else if width_sp < 5 {
                2
            } else if width_sp < 7 {
                3
            } else if width_sp < 10 {
                4
            } else if width_sp < 15 {
                5
            } else if width_sp < 20 {
                6
            } else {
                7
            };
            let interval = (start_line - end_line).abs().min(7) as usize;
            // // Port of `getSlopeConstraint` (`beamtremololayout.cpp:1157-1178`):
            // an inner note more extreme than both ends forces a flat beam.
            // `Beam-CrossM1`'s second group (lines 5,3,4,6) is flat in 4.7.4;
            // the old heuristic slanted it by 0.25 sp per stem.
            let all_lines: alloc::vec::Vec<i32> = group
                .iter()
                .filter_map(|&index| events[index].1.chord.as_ref())
                .flat_map(|chord| chord.notes.iter().map(|note| note.line))
                .collect();
            let flat = all_lines.iter().copied().min().is_some_and(|line| {
                if up {
                    line < start_line.min(end_line)
                } else {
                    line > start_line.max(end_line)
                }
            }) || all_lines.iter().copied().max().is_some_and(|line| {
                if up {
                    line > start_line.max(end_line)
                } else {
                    line < start_line.min(end_line)
                }
            });
            let slant = if flat {
                0
            } else {
                max_slope.min(MAX_SLOPES[interval]) * if up { 1 } else { -1 }
            };
            let start_dictator = if up {
                start_line < end_line
            } else {
                start_line > end_line
            };
            let mut dictator = if start_dictator { start_y } else { end_y };
            let mut pointer = if start_dictator { end_y } else { start_y };
            let dictator_is_start = start_dictator;
            let ascending = start_line > end_line;
            let backwards = (dictator > pointer)
                != if dictator_is_start {
                    ascending
                } else {
                    !ascending
                };
            if backwards {
                dictator = pointer - slant * 256;
            } else {
                pointer = dictator + slant * 256;
            }
            let y1 = if start_dictator { dictator } else { pointer };
            let y2 = if start_dictator { pointer } else { dictator };
            // // Port of `offsetBeamWithAnchorShortening`
            // (`beamtremololayout.cpp:213-286`): the anchors of a flat beam
            // are moved one beam width toward the beam when the shortening
            // rule cannot be satisfied. Measured on `Beam-CrossM1`'s second
            // group (lines 5,3,4,6): 4.7.4 puts the beam 1.25 sp above the
            // staff and gives stems 3.5/2.5/3/4 sp; without this step both
            // were half a beam width short.
            let flat_offset = if flat {
                if up {
                    -BEAM_WIDTH
                } else {
                    BEAM_WIDTH
                }
            } else {
                0
            };
            let beam_y = |x: i32| {
                y1 + (i64::from(y2 - y1) * i64::from(x - x1) / i64::from(x2 - x1)) as i32
                    + flat_offset
            };
            let max_beams = group
                .iter()
                .map(|&index| usize::from(events[index].1.beams))
                .max()
                .unwrap_or(0);
            for level in 0..max_beams {
                // One beam segment per maximal run of chords carrying this
                // level (`createBeamSegment`, beamlayout.cpp:1215-1216).
                let mut run: Vec<usize> = Vec::new();
                let flush = |run: &mut Vec<usize>, plan: &mut BeamPlan| {
                    if run.len() < 2 {
                        run.clear();
                        return;
                    }
                    let sx = stem_x(run[0]);
                    let ex = stem_x(*run.last().unwrap());
                    let offset = if up {
                        -(level as i32) * BEAM_DIST
                    } else {
                        (level as i32) * BEAM_DIST
                    };
                    plan.items.push(DisplayItem::beam(
                        ItemKind::Beam,
                        sx,
                        system_y + beam_y(sx) + offset - BEAM_WIDTH / 2,
                        ex,
                        system_y + beam_y(ex) + offset - BEAM_WIDTH / 2,
                        BEAM_WIDTH,
                    ));
                    for &index in run.iter() {
                        if events[index].1.chord.is_some() {
                            // `place_chord` measures the stem from the same
                            // anchor line used here; its far end is the beam's
                            // centre line at the stem's x (`tdraw.cpp:920-936`
                            // draws the thickness symmetrically around it).
                            let note_y = anchor_line(index) * HALF_SP;
                            let far_end = beam_y(stem_x(index)) + offset;
                            let length = if up {
                                (note_y - far_end).max(0)
                            } else {
                                (far_end - note_y).max(0)
                            };
                            plan.stems[index] = Some((up, length));
                        }
                    }
                    run.clear();
                };
                for &index in &group {
                    if events[index].1.beams as usize > level {
                        run.push(index);
                    } else {
                        flush(&mut run, &mut plan);
                    }
                }
                flush(&mut run, &mut plan);
            }
        }
    }
    plan
}

/// Absolute x of every event of a system, in the same order as
/// `place_measure` advances its cursor. Shared by beam layout and drawing so
/// both agree after justification.
fn system_events<'a>(
    atoms: &'a [MeasureAtom],
    system: &crate::breaking::SystemPlan,
    left: i32,
) -> Vec<(i32, &'a Atom)> {
    let mut events = Vec::new();
    let mut measure_x = left;
    for atom in &atoms[system.first..system.first + system.count] {
        let mut cursor = atom.events.first().map_or(0, |event| event.x);
        for (index, event) in atom.events.iter().enumerate() {
            events.push((measure_x + cursor, event));
            let width = if index + 1 < atom.events.len() {
                atom.events[index + 1].x - event.x
            } else {
                atom.width - event.x
            };
            let stretched =
                if !event.grace && matches!(event.kind, AtomKind::Chord | AtomKind::Rest { .. }) {
                    width * system.scale / 1024
                } else {
                    width
                };
            cursor += stretched;
        }
        measure_x += cursor;
    }
    events
}

/// Advance of one segment, stretched for chord/rest springs only.
/// // Port of `horizontalspacing.cpp:1087-1111` (`stretchSegmentsToWidth`).
fn segment_width(atom: &MeasureAtom, index: usize, scale: i32) -> i32 {
    let event = &atom.events[index];
    let width = if index + 1 < atom.events.len() {
        atom.events[index + 1].x - event.x
    } else {
        atom.width - event.x
    };
    if !event.grace && matches!(event.kind, AtomKind::Chord | AtomKind::Rest { .. }) {
        width * scale / 1024
    } else {
        width
    }
}

/// Cursor at the end of a measure's last segment (the bar line position).
fn measure_end(atom: &MeasureAtom, scale: i32) -> i32 {
    atom.events.first().map_or(0, |event| event.x)
        + (0..atom.events.len())
            .map(|index| segment_width(atom, index, scale))
            .sum::<i32>()
}

/// Right edge of a system's final bar line, used to clip its staff lines.
/// // Port of `tdraw.cpp`'s bar-line drawing with `endBarWidth` and
/// `barlineSeparation` (`styledef.cpp:147,149`).
fn system_right_edge(atom: &MeasureAtom, x: i32) -> i32 {
    match atom.end_bar.0 {
        3 => x + BARLINE_SEPARATION + THICK_BARLINE_WIDTH,
        2 | 1 => x + THIN_BARLINE_WIDTH / 2,
        _ => x,
    }
}

/// Place one measure's events at `x0` on the system at `system_y`.
///
/// // Port of `horizontalspacing.cpp:214-219` and `:1087-1111`: chord/rest
/// segment widths are stretched by the system's spring force; the header
/// segments keep their style-derived widths.
#[allow(clippy::too_many_arguments)] // page-local helper: out, atom, origin, scale and precomputed events
fn place_measure(
    out: &mut DisplayList,
    atom: &MeasureAtom,
    x0: i32,
    system_y: i32,
    scale: i32,
    events: &[(i32, &Atom)],
    event_offset: usize,
    plan: &BeamPlan,
    bar_height: i32,
    draw_end_bar: bool,
    lyrics_em: i32,
) -> i32 {
    // The first header event sits at its own x (e.g. clefLeftMargin); the
    // loop below then advances by the gap to the following event.
    let mut cursor = atom.events.first().map_or(0, |event| event.x);
    for (index, event) in atom.events.iter().enumerate() {
        let absolute_x = events
            .get(event_offset + index)
            .map_or(x0 + cursor, |item| item.0);
        place_event(
            out,
            event,
            absolute_x,
            x0 + cursor + segment_width(atom, index, scale),
            system_y,
            atom.mm_rest_count,
            plan.stems.get(event_offset + index).copied().flatten(),
            lyrics_em,
        );
        cursor += segment_width(atom, index, scale);
    }
    // // Port of `tdraw.cpp` bar-line drawing: thin lines use the style width,
    // double lines are separated by `barlineSeparation` (SMuFL).
    // // Port of `horizontalspacing.cpp:1148-1152`: a measure ends at the
    // last segment's right edge. The former -SEGMENT_WIDTH/2 offset placed
    // every bar line 0.75 sp too early.
    let bar_x = x0 + cursor;
    if draw_end_bar {
        place_barline(out, atom.end_bar, bar_x, system_y, bar_height);
    }
    cursor
}

fn place_barline(out: &mut DisplayList, code: crate::atoms::BarCode, x: i32, y: i32, height: i32) {
    let w = THIN_BARLINE_WIDTH;
    match code.0 {
        1 => out.push(DisplayItem::rect(
            ItemKind::BarLine,
            x - w / 2,
            y,
            w,
            height,
        )),
        2 => {
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - BARLINE_SEPARATION - w / 2,
                y,
                w,
                height,
            ));
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - w / 2,
                y,
                w,
                height,
            ));
        }
        3 => {
            // // Port of `tdraw.cpp:752-805`: the thin stroke sits at the
            // measure's right edge x; the thick stroke follows it by
            // `endBarDistance` + half of `endBarWidth`. `breaking` excludes
            // that 0.92 sp from the justified springs, so x is already
            // shifted left from the printable margin.
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x - THIN_BARLINE_WIDTH / 2,
                y,
                THIN_BARLINE_WIDTH,
                height,
            ));
            out.push(DisplayItem::rect(
                ItemKind::BarLine,
                x + BARLINE_SEPARATION,
                y,
                THICK_BARLINE_WIDTH,
                height,
            ));
        }
        _ => {}
    }
}

/// SMuFL key-signature positions on a five-line treble staff, in half-space
/// line units (top line = 0). // Port of `keysig.cpp`'s sharp/flat order and
/// `tdraw.cpp:2099` (`TDraw::draw(const KeySig*)`).
const SHARP_LINES: [i32; 7] = [0, 3, -1, 2, 5, 1, 4];
const FLAT_LINES: [i32; 7] = [4, 1, 5, 2, 6, 3, 0];

fn place_key_sig(out: &mut DisplayList, fifths: i8, prev_fifths: i8, x: i32, y: i32) {
    let mut gx = x;
    // // Port of `KeySig::layout` / `measurelayout.cpp:2597-2660`: when the
    // key changes, naturals cancel the accidentals that are no longer in
    // force before the new ones are drawn. `copypaste_parts` measure 3 goes
    // from four sharps back to C, so four naturals are required.
    let cancel_sharps = if prev_fifths > 0 {
        (prev_fifths - fifths.max(0)).clamp(0, 7) as usize
    } else {
        0
    };
    for index in (fifths.max(0) as usize)..(fifths.max(0) as usize + cancel_sharps) {
        let width = cancel_glyph(out, SHARP_LINES[index.min(6)], gx, y);
        gx += width + KEYSIG_ACCIDENTAL_DISTANCE;
    }
    let cancel_flats = if prev_fifths < 0 {
        (fifths.min(0) - prev_fifths).clamp(0, 7) as usize
    } else {
        0
    };
    for index in (fifths.min(0).unsigned_abs() as usize)
        ..(fifths.min(0).unsigned_abs() as usize + cancel_flats)
    {
        let width = cancel_glyph(out, FLAT_LINES[index.min(6)], gx, y);
        gx += width + KEYSIG_ACCIDENTAL_DISTANCE;
    }
    let count = fifths.unsigned_abs().min(7) as usize;
    for index in 0..count {
        let (line, name) = if fifths > 0 {
            (SHARP_LINES[index], "accidentalSharp")
        } else {
            (FLAT_LINES[index], "accidentalFlat")
        };
        out.glyph(ItemKind::Accidental, name, gx, y + line * HALF_SP);
        // // Port of `styledef.cpp:241` (`keysigAccidentalDistance` 0.3 sp):
        // one SMuFL glyph width plus a fixed gap between accidentals.
        let width = glyph(name)
            .map(|g| units_to_fixed(g.bbox[2] - g.bbox[0]))
            .unwrap_or(SP);
        gx += width + KEYSIG_ACCIDENTAL_DISTANCE;
    }
}

fn cancel_glyph(out: &mut DisplayList, line: i32, x: i32, y: i32) -> i32 {
    let name = "accidentalNatural";
    out.glyph(ItemKind::Accidental, name, x, y + line * HALF_SP);
    glyph(name)
        .map(|g| units_to_fixed(g.bbox[2] - g.bbox[0]))
        .unwrap_or(SP)
}

/// SMuFL name of one decimal digit (`timeSigSymIdsFromString`,
/// `engraving/dom/utils.cpp:954-970`).
fn time_sig_digit(value: u8) -> &'static str {
    match value {
        0 => "timeSig0",
        1 => "timeSig1",
        2 => "timeSig2",
        3 => "timeSig3",
        4 => "timeSig4",
        5 => "timeSig5",
        6 => "timeSig6",
        7 => "timeSig7",
        8 => "timeSig8",
        _ => "timeSig9",
    }
}

fn decimal_digits(mut value: u8) -> alloc::vec::Vec<u8> {
    if value == 0 {
        return alloc::vec![0];
    }
    let mut digits = alloc::vec::Vec::new();
    while value > 0 {
        digits.push(value % 10);
        value /= 10;
    }
    digits.reverse();
    digits
}

/// Advance of a glyph in fixed staff spaces (the font's `hmtx` advance).
fn glyph_advance(name: &str) -> i32 {
    glyph(name)
        .map(|item| units_to_fixed(item.advance))
        .unwrap_or(SP)
}

/// Width and height of a run of time-signature digits, as MuseScore's
/// `font->bbox(SymIdList, mag)` does (`tlayout.cpp:6154-6172`): the run's
/// width is the advances of all but the last glyph plus that glyph's own ink
/// width; its height is the tallest digit's ink height.
fn time_sig_rect(value: u8) -> (i32, i32) {
    let digits = decimal_digits(value);
    let mut width = 0;
    let mut height = 0;
    for (index, digit) in digits.iter().enumerate() {
        let name = time_sig_digit(*digit);
        let Some(item) = glyph(name) else {
            continue;
        };
        width += if index + 1 < digits.len() {
            units_to_fixed(item.advance)
        } else {
            units_to_fixed(item.bbox[2] - item.bbox[0])
        };
        height = height.max(units_to_fixed(item.bbox[3] - item.bbox[1]));
    }
    (width, height)
}

/// // Port of `TLayout::layoutTimeSig` (`tlayout.cpp:6044-6185`) and
/// `TDraw::draw(const TimeSig*)` (`tdraw.cpp:3137-3153`): every decimal digit
/// becomes its own SMuFL symbol, the two runs are centred on one another, and
/// their origins sit symmetrically around the middle staff line. `numDist`
/// (`timeSigNormalNumDist`) is zero by default (`styledef.cpp:203`).
/// The old code skipped values above 9, leaving a lone "8" for 12/8.
fn place_time_sig(out: &mut DisplayList, numerator: u8, denominator: u8, x: i32, y: i32) {
    let (num_width, num_height) = time_sig_rect(numerator);
    let (den_width, den_height) = time_sig_rect(denominator);
    let middle = 2 * SP; // five lines, line distance 1 (`tlayout.cpp:6097`)
    let (num_x, den_x) = if num_width >= den_width {
        (0, (num_width - den_width) / 2)
    } else {
        ((den_width - num_width) / 2, 0)
    };
    let num_y = middle - num_height / 2;
    let den_y = middle + den_height / 2;
    for (value, offset, origin_y) in [(numerator, num_x, num_y), (denominator, den_x, den_y)] {
        let mut pen = x + offset;
        for digit in decimal_digits(value) {
            let name = time_sig_digit(digit);
            out.glyph(ItemKind::TimeSig, name, pen, y + origin_y);
            pen += glyph_advance(name);
        }
    }
}

/// Font em in fixed staff spaces: point size converted with the score's
/// spatium (`styledef.cpp:1209-1223`, `TextStyle::fontSize`).
pub(crate) fn text_em(spec: &PageSpec, font_size: f64) -> i32 {
    ((font_size * 25.4 / 72.0) / spec.spatium_mm * SP as f64) as i32
}

/// Width of a text run with real Edwin advances (`horizontalspacing.cpp` uses
/// the same item widths to size its segments).
pub(crate) fn text_run_width(text: &str, em: i32, bold: bool) -> i32 {
    text.chars()
        .filter_map(|character| u16::try_from(character).ok())
        .map(|codepoint| text_advance(codepoint, em, bold))
        .sum()
}

/// Baseline offset from the top of a text run (Edwin cap height is about
/// 736 font units; measured on the 4.7.4 title SVG).
pub(crate) fn text_cap_height(text: &str, em: i32, bold: bool) -> i32 {
    text.chars()
        .filter_map(|character| u16::try_from(character).ok())
        .filter_map(|codepoint| text_glyph(codepoint, bold))
        .map(|glyph| i32::from(glyph.bbox[3]) * em / i32::from(UPEM))
        .max()
        .unwrap_or(em * 3 / 4)
}

/// Emits one Edwin glyph per character. `align` is 0 (left), 1 (centre) or
/// 2 (right); the pen starts at `x` with a top at `y` (not the baseline).
pub(crate) fn place_text(
    out: &mut DisplayList,
    text: &str,
    x: i32,
    y: i32,
    em: i32,
    bold: bool,
    align: u8,
) {
    place_text_kind(out, ItemKind::Text, text, x, y, em, bold, align);
}

/// Same as [`place_text`], with an explicit display-list class (used by
/// instrument names and lyrics).
#[allow(clippy::too_many_arguments)] // page-local helper: list, style and text run
pub(crate) fn place_text_kind(
    out: &mut DisplayList,
    kind: ItemKind,
    text: &str,
    x: i32,
    y: i32,
    em: i32,
    bold: bool,
    align: u8,
) {
    let width = text_run_width(text, em, bold);
    let mut pen = match align {
        1 => x - width / 2,
        2 => x - width,
        _ => x,
    };
    let baseline = y + text_cap_height(text, em, bold);
    for character in text.chars() {
        let Some(codepoint) = u16::try_from(character).ok() else {
            continue;
        };
        if text_glyph(codepoint, bold).is_some() {
            out.push(DisplayItem::text(kind, codepoint, pen, baseline, em, bold));
        }
        pen += text_advance(codepoint, em, bold);
    }
}

/// Lyrics below their staff, as plain Edwin text. MuseScore's
/// `LyricsLayout::layout` positions the syllable under its chord, with the
/// default `lyricsPlacement: below` and `lyricsPosBelow` 3 sp below the
/// bottom staff line (`styledef.cpp:73-74`, `lyricslayout.cpp:45-170`).
/// Full text metrics/autoplace arrive in P4; here one Edwin glyph per
/// character is drawn at the segment's shared x.
pub(crate) fn place_lyrics(
    out: &mut DisplayList,
    lyrics: &[LyricAtom],
    x: i32,
    staff_y: i32,
    em: i32,
) {
    for lyric in lyrics {
        let text = &lyric.text;
        if text.is_empty() {
            continue;
        }
        // First verse baseline 3 sp below the bottom staff line; following
        // verses are one text line lower (`lyricsPosBelow`, `lyricsLineHeight`).
        let baseline = staff_y + 4 * SP + 3 * SP + lyric.verse as i32 * LYRIC_LINE_HEIGHT;
        let top = baseline - text_cap_height(text, em, false);
        // The 4.7.4 style centres syllables under the notehead
        // (`lyricsEvenAlign`/`lyricsOddAlign`, `styledef.cpp:109,124`).
        // A melisma-only syllable with no text of its own would be left
        // aligned (`lyricsMelismaAlign`), which this plain-text subset does
        // not emit separately.
        place_text_kind(out, ItemKind::Text, text, x + SP / 2, top, em, false, 1);
    }
}

/// Hyphens and melisma extender lines of one system, ported from
/// `LyricsLayout::layoutDashes` (`lyricslayout.cpp:206-366`). The dash width
/// is capped at `lyricsDashMaxLength` 0.6 sp and their count is derived from
/// `lyricsDashMaxDistance` 16 sp; the syllable and melisma styles place them
/// halfway up the text's cap height.
fn plan_lyric_lines(
    events: &[(i32, &Atom)],
    staff_y: i32,
    right: i32,
    em: i32,
) -> Vec<DisplayItem> {
    let mut items = Vec::new();
    for (index, (x, event)) in events.iter().enumerate() {
        for lyric in &event.lyrics {
            if lyric.text.is_empty() {
                continue;
            }
            let width = text_run_width(&lyric.text, em, false);
            let cap = text_cap_height(&lyric.text, em, false);
            let baseline = staff_y + 7 * SP + lyric.verse as i32 * LYRIC_LINE_HEIGHT;
            let y = baseline - cap / 2 - LYRIC_LINE_WIDTH / 2;
            let text_centre = *x + SP / 2;
            let start = if lyric.melisma {
                text_centre + width / 2 + LYRIC_MELISMA_PAD
            } else {
                text_centre + width / 2 + LYRIC_DASH_MIN / 2
            };
            if matches!(
                lyric.syllabic,
                LyricsSyllabic::Begin | LyricsSyllabic::Middle
            ) {
                items.push(DisplayItem::rect(
                    ItemKind::Text,
                    start - LYRIC_DASH_MIN / 2,
                    y,
                    LYRIC_DASH_MIN,
                    LYRIC_LINE_WIDTH,
                ));
            }
            if lyric.melisma {
                let end = events[index + 1..]
                    .iter()
                    .find(|(_, later)| later.lyrics.iter().any(|later| later.verse == lyric.verse))
                    .map_or(right, |(next, _)| *next - LYRIC_MELISMA_PAD)
                    .max(start + LYRIC_LINE_WIDTH);
                let length = end - start;
                let dash_width = length.min(LYRIC_DASH_MAX);
                let count = ((length / (16 * SP)).max(1) as usize).max(1);
                let step = length / count as i32;
                for dash in 0..count {
                    let centre = start + step * dash as i32 + step / 2;
                    items.push(DisplayItem::rect(
                        ItemKind::Text,
                        centre - dash_width / 2,
                        y,
                        dash_width,
                        LYRIC_LINE_WIDTH,
                    ));
                }
            }
        }
    }
    items
}

/// Title-frame texts (`VBox` in the .mscx, `score.engraving.credits` in the
/// pivot model), laid out at the top of the page.
/// // Port of `boxlayout.cpp` and `textlayout.cpp`, with the style defaults
/// of `styledef.cpp` (title centred, subtitle centred, composer right).
fn place_credits(out: &mut DisplayList, credits: &[Credit], spec: &PageSpec, page: usize) {
    let top = spec.margin_top;
    let visible: Vec<_> = credits
        .iter()
        .filter(|credit| credit.page as usize == page + 1 && !credit.text.plain().is_empty())
        .collect();
    // The VBox lists the subtitle before the title in `Beam-CrossM1`; they
    // must be placed in reading order, not in source order.
    let title_em = visible
        .iter()
        .find(|credit| matches!(credit.style, CreditStyle::Title))
        .map(|credit| {
            text_em(
                spec,
                credit
                    .font_size
                    .unwrap_or_else(|| credit.style.default_font_size()),
            )
        })
        .unwrap_or(SP * 4);
    for credit in &visible {
        let text = credit.text.plain();
        let size = credit
            .font_size
            .unwrap_or_else(|| credit.style.default_font_size());
        let em = text_em(spec, size);
        let (x, align) = match credit.justify {
            CreditJustify::Left => (spec.margin_left, 0),
            CreditJustify::Right => (spec.margin_left + spec.printable_width, 2),
            CreditJustify::Center => (spec.margin_left + spec.printable_width / 2, 1),
        };
        let y = match credit.style {
            CreditStyle::Title => top,
            CreditStyle::Subtitle => top + title_em + SP / 2,
            CreditStyle::Composer | CreditStyle::Lyricist => {
                top + spec.title_frame_height.saturating_sub(7 * SP + em)
            }
            CreditStyle::Other(_) => top,
        };
        // A VBox text can hold several lines separated by `<br/>`
        // (`TextBase::createBlocks`). Each line keeps its style's horizontal
        // alignment; bottom-aligned styles stack their lines upward from the
        // frame's bottom edge.
        let lines: alloc::vec::Vec<_> = text.split('\n').filter(|line| !line.is_empty()).collect();
        let count = lines.len().max(1) as i32;
        for (index, line) in lines.iter().enumerate() {
            let line_y = match credit.style {
                CreditStyle::Composer | CreditStyle::Lyricist => {
                    y - (count - 1 - index as i32) * em
                }
                _ => y + index as i32 * em,
            };
            place_text(out, line, x, line_y, em, false, align);
        }
    }
}

/// Ledger lines: // Port of `chordlayout.cpp:1310-1314, 1355-1360, 1394-1419`
/// — notes outside `[-1, lineBelow+1]` need lines on every even line between
/// the note and the staff, extended by `ledgerLineLength` on both sides.
fn place_ledger_lines(out: &mut DisplayList, head_x: i32, head_w: i32, line: i32, y: i32) {
    if (-1..=9).contains(&line) {
        return;
    }
    if line < 0 {
        let mut ledger = (line + 1) & !1; // round toward zero to an even line
        while ledger < 0 {
            out.push(DisplayItem::rect(
                ItemKind::LedgerLine,
                head_x - LEDGER_EXTENSION,
                y + ledger * HALF_SP - LEDGER_WIDTH / 2,
                head_w + 2 * LEDGER_EXTENSION,
                LEDGER_WIDTH,
            ));
            ledger += 2;
        }
    } else {
        let mut ledger = line & !1;
        while ledger > 8 {
            out.push(DisplayItem::rect(
                ItemKind::LedgerLine,
                head_x - LEDGER_EXTENSION,
                y + ledger * HALF_SP - LEDGER_WIDTH / 2,
                head_w + 2 * LEDGER_EXTENSION,
                LEDGER_WIDTH,
            ));
            ledger -= 2;
        }
    }
}

/// // Port of `tlayout.cpp:5180-5201` and `stemlayout.cpp:220-228`: the
/// near end of a stem is the head's SMuFL stem anchor, not the note line.
/// The constant is calibrated against the 4.7.4 SVG for a normal head
/// (`stemUpSE` 40 / `stemDownNW` -42 font units); other heads shift by the
/// difference of their own anchors.
fn stem_anchor_offset(head: &str, up: bool) -> i32 {
    let (name, baseline) = if up {
        ("stemUpSE", 40)
    } else {
        ("stemDownNW", -42)
    };
    let units = glyph(head)
        .and_then(|g| anchor(g, name))
        .map(|a| i32::from(a.1))
        .unwrap_or(baseline);
    let offset = STEM_ANCHOR_OFFSET + (units - baseline) * 1024 / 250;
    if up {
        -offset
    } else {
        offset
    }
}

fn place_chord(
    out: &mut DisplayList,
    chord: &ChordAtom,
    x: i32,
    y: i32,
    beam: Option<(bool, i32)>,
) {
    // // Port of `ChordLayout::layoutChords2` (`chordlayout.cpp:2309-2410`):
    // notes on the same or an adjacent line conflict, so the head on the
    // stem side alternates to the opposite side of the stem ("seconds").
    let up = beam.map_or(chord.stem_up, |(up, _)| up);
    let mut mirror = false;
    let mut mirrored: alloc::vec::Vec<bool> = alloc::vec::Vec::new();
    for (index, note) in chord.notes.iter().enumerate() {
        if index > 0 && note.line - chord.notes[index - 1].line <= 1 {
            mirror = !mirror;
        }
        mirrored.push(mirror);
    }
    for (note_index, note) in chord.notes.iter().enumerate() {
        let head = glyph(note.head).unwrap();
        let head_width = units_to_fixed(head.bbox[2]);
        let head_x = if mirrored[note_index] {
            if up {
                x - head_width
            } else {
                x + head_width
            }
        } else {
            x
        };
        let ny = y + note.line * HALF_SP;
        // // Port of `accidentalslayout.cpp:286-305` (`stackAccidental`) and
        // `:745-767` (`minAccidentalToChordDistance`): the accidental's right
        // edge is separated from the notehead's left edge by
        // `accidentalDistance`, so it is drawn immediately **before** its
        // head. The previous 3.5 sp offset placed it after the preceding
        // notehead of the system (`copypaste_parts`, measure 1).
        if let Some(name) = note.accidental {
            let width = glyph(name)
                .map(|g| units_to_fixed(g.bbox[2] - g.bbox[0]))
                .unwrap_or(SP);
            out.glyph(
                ItemKind::Accidental,
                name,
                head_x - width - ACCIDENTAL_DISTANCE,
                ny,
            );
        }
        let head_w = head_width;
        // // Port of `tlayout.cpp`/`tdraw.cpp`: the notehead origin is at the
        // left edge of the head, vertically centred on its staff line.
        out.path(ItemKind::Notehead, note.head, head_x, ny);
        place_ledger_lines(out, head_x, head_w, note.line, y);

        // Dots: // Port of `chordlayout.cpp:2490-2562` (`placeDots`) — a note
        // on a line moves its dot into the adjacent space, up for an up stem.
        if chord.dots > 0 {
            let dot_offset = if note.line % 2 == 0 {
                if chord.stem_up {
                    -1
                } else {
                    1
                }
            } else {
                0
            };
            for index in 0..usize::from(chord.dots) {
                let dx = head_x + head_w + DOT_NOTE_DISTANCE + index as i32 * DOT_DOT_DISTANCE;
                let dy = y + (note.line + dot_offset) * HALF_SP;
                out.glyph(ItemKind::Dot, "augmentationDot", dx, dy);
            }
        }
    }

    if chord.notes.is_empty() || !crate::atoms::has_stem(chord.duration) {
        return;
    }
    let top = chord.notes.iter().map(|n| n.line).min().unwrap();
    let bottom = chord.notes.iter().map(|n| n.line).max().unwrap();
    let head = glyph(chord.notes[0].head).unwrap();
    let head_w = units_to_fixed(head.bbox[2]);
    let stem_len = beam.map_or(chord.stem_len, |(_, length)| length);
    let anchor_line = if up { bottom } else { top };
    let ny = y + anchor_line * HALF_SP;
    let stem_x = if up {
        x + head_w - STEM_WIDTH / 2
    } else {
        x - STEM_WIDTH / 2
    };
    // // Port of `tlayout.cpp:5180-5187` (`TLayout::layoutStem`): the drawn
    // stem starts at the notehead's SMuFL stem anchor, not at the note's
    // centre. SMuFL anchor y is up-positive; our page coordinates are
    // down-positive, hence the sign flip. `stemlayout.cpp:227-242`
    // (`StemLayout::stemPos`) supplies the horizontal side of the stem.
    let anchor_head = if up {
        chord.notes.first()
    } else {
        chord.notes.last()
    };
    let anchor_y = stem_anchor_offset(anchor_head.map_or("noteheadBlack", |note| note.head), up);
    // `graceNoteMag` 0.7 scales the head and its stem anchor
    // (`beamlayout.cpp:339-341`, `styledef.cpp:474`).
    let anchor_y = if chord.grace {
        anchor_y * 7 / 10
    } else {
        anchor_y
    };
    // The far end stays at `note ± stemLength`; only the notehead end moves
    // to the anchor (`tlayout.cpp:5198-5201` computes y2 independently).
    let (sy, ey) = if up {
        (ny - stem_len, ny + anchor_y)
    } else {
        (ny + anchor_y, ny + stem_len)
    };
    // // Port of `stemlayout.cpp:227-242` and `tlayout.cpp:5143-5175`: a
    // stem is a rectangle from the notehead anchor to `stemLength` away.
    out.push(DisplayItem::rect(
        ItemKind::Stem,
        stem_x,
        sy,
        STEM_WIDTH,
        ey - sy,
    ));
    if beam.is_none() {
        if let Some(name) = flag_name(chord.duration, up) {
            let _ = (top, bottom);
            let flag_y = if up { sy } else { ey };
            out.glyph(ItemKind::Flag, name, stem_x, flag_y);
        }
    }
}

#[allow(clippy::too_many_arguments)] // page-local helper: item, bounds, staff y and beam override
fn place_event(
    out: &mut DisplayList,
    event: &Atom,
    x: i32,
    right: i32,
    y: i32,
    mm_rest_count: u16,
    beam: Option<(bool, i32)>,
    lyrics_em: i32,
) {
    match event.kind {
        AtomKind::Clef { name } => {
            // Leland's clef origin sits near the second staff line; the
            // horizontal margin is `clefLeftMargin` (styledef.cpp:174).
            out.glyph(ItemKind::Clef, name, x, y + 2 * SP);
        }
        AtomKind::KeySig {
            fifths,
            prev_fifths,
        } => place_key_sig(out, fifths, prev_fifths, x, y),
        AtomKind::TimeSig {
            numerator,
            denominator,
        } => place_time_sig(out, numerator, denominator, x, y),
        AtomKind::Chord => {
            if let Some(chord) = &event.chord {
                place_chord(out, chord, x, y, beam);
            }
            if !event.lyrics.is_empty() {
                place_lyrics(out, &event.lyrics, x + SP / 2, y, lyrics_em);
            }
        }
        AtomKind::Rest {
            duration,
            dots,
            measure,
        } => {
            if !event.lyrics.is_empty() {
                place_lyrics(out, &event.lyrics, x + SP / 2, y, lyrics_em);
            }
            // // Port of `RestLayout::computeVoiceOffset`
            // (`restlayout.cpp:694-765`): in a multi-voice measure the rests
            // of voices 2 and 4 move one line space down, those of voices 1
            // and 3 one up, so they do not collide with the other voice.
            let voice_offset = if event.voice.is_multiple_of(2) { 0 } else { SP };
            let name = rest_name(duration);
            let rest_y = y + HALF_SP + voice_offset;
            // `getMeasureStartEndPos` uses the previous segment's right
            // edge, i.e. the segment x minus `barNoteDistance`
            // (`measurelayout.cpp:1477-1485`), not its left edge.
            let x1 = x - BAR_NOTE_DISTANCE;
            if mm_rest_count > 1 {
                // // Port of `MeasureLayout::layoutMeasure`
                // (`measurelayout.cpp:1477-1492`, `:2997-3005`): an MMRest
                // is a thick bar centred vertically in the staff, inset by
                // `multiMeasureRestMargin` (1.25 sp) on both sides, with
                // vertical strokes of `mmRestHBarVStrokeHeight` (2 sp).
                let margin = 5 * SP / 4;
                let left = x1 + margin;
                let width = (right - x1 - 2 * margin).max(SP / 2);
                let mid = y + 2 * SP;
                out.push(DisplayItem::rect(
                    ItemKind::Rest,
                    left,
                    mid - 7 * SP / 20,
                    width,
                    7 * SP / 10,
                ));
                for end in [left, left + width - SP / 5] {
                    out.push(DisplayItem::rect(
                        ItemKind::Rest,
                        end,
                        mid - SP,
                        SP / 5,
                        2 * SP,
                    ));
                }
                return;
            }
            let width = glyph(name)
                .map(|glyph| units_to_fixed(glyph.bbox[2] - glyph.bbox[0]))
                .unwrap_or(SP);
            // // Port of `measurelayout.cpp:1501-1512`: a full-measure rest
            // is centred in the free space between the preceding segment and
            // the bar line, not drawn at the segment's left edge.
            let rest_x = if measure {
                x1 + ((right - x1 - width) / 2).max(0)
            } else {
                x
            };
            out.glyph(ItemKind::Rest, name, rest_x, rest_y);
            for index in 0..usize::from(dots) {
                out.glyph(
                    ItemKind::Dot,
                    "augmentationDot",
                    rest_x + 2 * SP + index as i32 * DOT_DOT_DISTANCE,
                    rest_y,
                );
            }
        }
    }
}

/// Builds the display list of one page from the atoms and the precomputed
/// breaking plan. `page(n)` only reads the systems assigned to that page.
pub fn build_page(
    atoms: &[Vec<MeasureAtom>],
    spec: &PageSpec,
    systems: &[crate::breaking::SystemPlan],
    staffs: &[crate::staffs::StaffInfo],
    credits: &[Credit],
    page: usize,
) -> DisplayList {
    let mut out = DisplayList::new(page);
    place_credits(&mut out, credits, spec, page);
    let (staff_y, _) = crate::staffs::system_geometry(staffs, spec);
    let lyrics_em = text_em(spec, spec.lyric_font_size);
    for system in systems.iter().filter(|system| system.page == page) {
        let sy = system.y;
        // // Port of `pagelayout.cpp:524-613` and `styledef.cpp:540`: a first
        // system after a title frame starts `firstSystemIndentationValue`
        // to the right and ends at the same right margin.
        let indent = if system.first == 0 {
            spec.first_system_indent
        } else {
            0
        };
        let left = spec.margin_left + indent;
        let staff_width = spec.printable_width - indent;
        // // Port of `MeasureLayout`/`measurelayout.cpp`: measure numbers are
        // drawn above the first measure of every system (except measure 1,
        // `showMeasureNumberOne` false by default), centred on the staff's
        // left edge (`measureNumberFontSize` 8 pt, styledef.cpp:1210).
        let first = &atoms[0][system.first];
        if first.number > 0 {
            let label = alloc::format!("{}", first.number + 1);
            place_text(
                &mut out,
                &label,
                left,
                sy - 3 * SP,
                text_em(spec, 8.0),
                false,
                1,
            );
        }
        let groups = crate::decor::barline_groups(staffs);
        crate::decor::place_brackets(&mut out, staffs, system, left);
        crate::decor::place_instrument_names(
            &mut out,
            spec,
            staffs,
            system,
            left,
            system.first == 0 && system.page == 0,
        );
        crate::decor::place_initial_barline(&mut out, staffs, &groups, system, left);
        // // Port of `SystemLayout::layoutSystem` (`systemlayout.cpp:2477-2582`):
        // each visible staff of the system is laid out in turn, with its own
        // clef/key/time and its own events at the shared segment x positions.
        for (staff_index, staff_atoms) in atoms.iter().enumerate() {
            let span = groups
                .iter()
                .find(|&&(first, last)| staff_index >= first && staff_index <= last);
            let (bar_height, draw_end_bar) = match span {
                Some(&(first, last)) => {
                    let top = system.staff_offsets.get(first).copied().unwrap_or(0);
                    let bottom =
                        system.staff_offsets.get(last).copied().unwrap_or(0) + STAFF_HEIGHT;
                    (bottom - top, staff_index == first)
                }
                None => (STAFF_HEIGHT, true),
            };
            let y = sy
                + system
                    .staff_offsets
                    .get(staff_index)
                    .copied()
                    .or_else(|| staff_y.get(staff_index).copied())
                    .unwrap_or(0);
            // // Port of `SystemLayout`/`tdraw.cpp`: the staff lines end at the
            // system's last bar line. A short final system (below
            // `lastSystemFillLimit`, `systemlayout.cpp:459-489`) is not
            // stretched to the right margin.
            let mut end_x = left;
            for atom in &staff_atoms[system.first..system.first + system.count] {
                end_x += measure_end(atom, system.scale);
            }
            let last = &staff_atoms[system.first + system.count - 1];
            let right = (system_right_edge(last, end_x) - left).clamp(0, staff_width);
            // // Port of `tdraw.cpp`'s staff-line drawing: five pixel-snapped
            // rectangles, thickness `staffLineWidth`.
            for line in 0..5 {
                out.push(DisplayItem::rect(
                    ItemKind::StaffLine,
                    left,
                    y + line * SP - STAFF_LINE_WIDTH / 2,
                    right,
                    STAFF_LINE_WIDTH,
                ));
            }
            let events = system_events(staff_atoms, system, left);
            let mut voices = alloc::vec::Vec::new();
            for (_, event) in &events {
                if !voices.contains(&event.voice) {
                    voices.push(event.voice);
                }
            }
            let multi_voice = voices.len() > 1;
            let plan = plan_beams(&events, multi_voice, y);
            for item in &plan.items {
                out.push(*item);
            }
            for item in plan_lyric_lines(&events, y, left + right, lyrics_em) {
                out.push(item);
            }
            let mut x = left;
            let mut event_offset = 0;
            for atom in &staff_atoms[system.first..system.first + system.count] {
                x += place_measure(
                    &mut out,
                    atom,
                    x,
                    y,
                    system.scale,
                    &events,
                    event_offset,
                    &plan,
                    bar_height,
                    draw_end_bar,
                    lyrics_em,
                );
                event_offset += atom.events.len();
            }
            // // Port of `measurelayout.cpp` / `tdraw.cpp`: a plain start bar
            // line is not drawn at the beginning of a system (the 4.7.4 SVG
            // has none). Repeats and double/final start bar lines are drawn.
            let start_bar = staff_atoms[system.first].start_bar;
            if start_bar.0 > 1 && draw_end_bar {
                place_barline(&mut out, start_bar, left, y, bar_height);
            }
        }
    }
    out
}

/// Iterates over all items of a page in deterministic order (used by tests
/// and the oracles).
pub fn items(
    atoms: &[Vec<MeasureAtom>],
    spec: &PageSpec,
    systems: &[crate::breaking::SystemPlan],
    staffs: &[crate::staffs::StaffInfo],
    page: usize,
) -> Vec<DisplayItem> {
    build_page(atoms, spec, systems, staffs, &[], page).items
}

/// Exposed for the raster backend: the quadratic outline of a display item.
pub fn geometry_of(item: &DisplayItem) -> Geometry {
    item.geometry
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    use crate::atoms::NoteAtom;

    fn page(atoms: &[MeasureAtom], spec: &PageSpec, number: usize) -> DisplayList {
        let systems = crate::breaking::compute(atoms, spec);
        build_page(&[atoms.to_vec()], spec, &systems, &[], &[], number)
    }

    fn page_count(atoms: &[MeasureAtom], spec: &PageSpec) -> usize {
        crate::breaking::page_count(&crate::breaking::compute(atoms, spec))
    }

    fn atom(line: i32) -> MeasureAtom {
        MeasureAtom {
            number: 0,
            width: 8 * SEGMENT_WIDTH,
            spring: 8 * 1024,
            events: vec![Atom {
                x: 4 * SP,
                kind: AtomKind::Chord,
                chord: Some(ChordAtom {
                    duration: 4,
                    dots: 0,
                    grace: false,
                    stem_up: true,
                    stem_len: 3584,
                    notes: vec![NoteAtom {
                        line,
                        head: "noteheadBlack",
                        accidental: None,
                    }],
                }),
                voice: 0,
                measure: 0,
                beams: 0,
                beam_mode: scoretk_core::model::BeamMode::Auto,
                beam_start: false,
                beam_dir: None,
                grace: false,
                ..Default::default()
            }],
            start_bar: crate::atoms::BarCode(1),
            end_bar: crate::atoms::BarCode(1),
            line_break: false,
            clef_name: "gClef",
            key_fifths: 0,
            mm_rest_count: 1,
            ..Default::default()
        }
    }

    #[test]
    fn multiline_credits_keep_their_style_alignment() {
        let mut credit = Credit::new(
            scoretk_core::model::RichText::from_markup("Alone<br/>Together"),
            CreditStyle::Composer,
        );
        credit.page = 1;
        credit.justify = CreditJustify::Right;
        let spec = PageSpec::default();
        let mut out = DisplayList::new(0);
        place_credits(&mut out, &[credit], &spec, 0);
        let texts: alloc::vec::Vec<_> = out
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Text)
            .collect();
        assert!(texts.len() >= 2);
        // Both lines are right-aligned on the same right margin, and the
        // second line sits below the first.
        let first = texts[0].origin();
        let last = texts[texts.len() - 1].origin();
        assert!(last.1 > first.1);
        assert!(first.0 <= spec.margin_left + spec.printable_width);
    }

    #[test]
    fn time_signature_draws_every_digit_of_a_compound_meter() {
        // `tlayout.cpp:6154-6185`: all digits of 12 are emitted above the 8,
        // centred on it and stacked around the middle staff line.
        let mut out = DisplayList::new(0);
        place_time_sig(&mut out, 12, 8, 4 * SP, 5 * SP);
        let time: alloc::vec::Vec<_> = out
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::TimeSig)
            .collect();
        assert_eq!(time.len(), 3);
        let (_, upper_y) = time[0].origin();
        let (_, lower_y) = time[2].origin();
        assert!(
            upper_y > 5 * SP + SP / 2 && upper_y < 5 * SP + 2 * SP,
            "upper={upper_y} lower={lower_y}"
        );
        assert!(lower_y > upper_y + SP);
        let (x1, _) = time[0].origin();
        let (x2, _) = time[1].origin();
        assert!(x2 > x1, "the digits of the numerator must not overlap");
        // 6/8 and 3/2 stay one digit per row.
        for (numerator, denominator) in [(6, 8), (3, 2)] {
            let mut out = DisplayList::new(0);
            place_time_sig(&mut out, numerator, denominator, 4 * SP, 5 * SP);
            assert_eq!(out.count(ItemKind::TimeSig), 2);
        }
    }

    #[test]
    fn lyrics_are_centred_below_the_staff() {
        let mut out = DisplayList::new(0);
        place_lyrics(
            &mut out,
            &[LyricAtom {
                verse: 0,
                text: String::from("Mi"),
                syllabic: LyricsSyllabic::Begin,
                melisma: false,
            }],
            10 * SP,
            5 * SP,
            2 * SP,
        );
        assert_eq!(out.count(ItemKind::Text), 2); // one Edwin glyph per character
        for item in &out.items {
            let (_, y) = item.origin();
            assert!(y >= 5 * SP + 4 * SP, "lyrics must sit below the staff");
        }
    }

    #[test]
    fn lyric_syllables_get_hyphens_and_melisma_lines() {
        let lyric = |text: &str, syllabic, melisma| LyricAtom {
            verse: 0,
            text: String::from(text),
            syllabic,
            melisma,
        };
        let first = Atom {
            lyrics: vec![lyric("Mi", LyricsSyllabic::Begin, false)],
            ..Atom::at_segment(0, AtomKind::Chord, 0)
        };
        let second = Atom {
            lyrics: vec![lyric("nuit", LyricsSyllabic::End, true)],
            ..Atom::at_segment(0, AtomKind::Chord, 1)
        };
        let third = Atom {
            lyrics: vec![lyric("chré", LyricsSyllabic::Begin, false)],
            ..Atom::at_segment(0, AtomKind::Chord, 2)
        };
        let events = vec![(4 * SP, &first), (10 * SP, &second), (16 * SP, &third)];
        let lines = plan_lyric_lines(&events, 5 * SP, 20 * SP, 2 * SP);
        assert_eq!(lines.len(), 3); // two hyphens, one melisma dash
        for item in &lines {
            let (_, y) = item.origin();
            assert!(y > 5 * SP + 4 * SP, "lyric lines sit below the staff");
        }
    }

    #[test]
    fn page_has_five_staff_lines_and_the_event_items() {
        let spec = PageSpec::default();
        let list = page(&[atom(4)], &spec, 0);
        assert_eq!(list.count(ItemKind::StaffLine), 5);
        assert_eq!(list.count(ItemKind::Notehead), 1);
        assert_eq!(list.count(ItemKind::Stem), 1);
        // A plain start bar line is implicit in MuseScore; only the end bar
        // line of the measure is drawn.
        assert_eq!(list.count(ItemKind::BarLine), 1);
    }

    #[test]
    fn full_measure_rest_is_centred_and_mmrest_is_a_bar() {
        let spec = PageSpec {
            printable_width: 1000 * SP,
            ..PageSpec::default()
        };
        let mut rest = atom(4);
        rest.events = vec![Atom {
            x: 4 * SP,
            kind: AtomKind::Rest {
                duration: 2,
                dots: 0,
                measure: true,
            },
            chord: None,
            voice: 0,
            measure: 0,
            beams: 0,
            beam_mode: BeamMode::Auto,
            beam_start: false,
            beam_dir: None,
            grace: false,
            ..Default::default()
        }];
        rest.width = 40 * SP;
        let list = page(&[rest.clone()], &spec, 0);
        let glyph = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Rest)
            .unwrap();
        let (x0, _, x1, _) = glyph.bbox(&list).unwrap();
        let bar = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::BarLine)
            .unwrap()
            .origin()
            .0;
        assert!(
            ((x0 + x1) / 2 - (4 * SP - BAR_NOTE_DISTANCE + spec.margin_left + bar) / 2).abs()
                <= SP / 10,
            "measure rest is not centred in its measure"
        );
        // An MMRest is drawn as a centred bar plus two vertical end strokes.
        rest.mm_rest_count = 3;
        let list = page(&[rest], &spec, 0);
        let bars: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Rest)
            .collect();
        assert_eq!(bars.len(), 3);
        let Geometry::Rect { x, w, .. } = bars[0].geometry else {
            panic!("MMRest bar must be a rectangle");
        };
        assert!(w > 10 * SP, "the MMRest bar spans the measure");
        assert_eq!(
            x,
            spec.margin_left + 4 * SP - BAR_NOTE_DISTANCE + 5 * SP / 4
        );
    }

    #[test]
    fn staff_lines_stop_at_the_last_bar_line() {
        // A short final system is left unstretched (`shouldBeJustified`,
        // `systemlayout.cpp:459-489`). Its staff lines must stop at the final
        // bar line, not run to the printable margin.
        let spec = PageSpec {
            printable_width: 1000 * SP,
            ..PageSpec::default()
        };
        let list = page(&[atom(4)], &spec, 0);
        let staff = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::StaffLine)
            .unwrap();
        let Geometry::Rect { x, w, .. } = staff.geometry else {
            panic!("staff line must be a rectangle");
        };
        let bars: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::BarLine)
            .collect();
        let last_bar = bars
            .last()
            .and_then(|item| match item.geometry {
                Geometry::Rect { x, w, .. } => Some(x + w),
                _ => None,
            })
            .unwrap();
        assert!(x + w < spec.margin_left + spec.printable_width);
        assert!((x + w - last_bar).abs() <= THICK_BARLINE_WIDTH);
    }

    #[test]
    fn high_note_gets_ledger_lines_only_outside_the_staff() {
        let spec = PageSpec::default();
        let inside = page(&[atom(4)], &spec, 0);
        assert_eq!(inside.count(ItemKind::LedgerLine), 0);
        let outside = page(&[atom(-2)], &spec, 0);
        assert_eq!(outside.count(ItemKind::LedgerLine), 1);
        let low = page(&[atom(12)], &spec, 0);
        assert_eq!(low.count(ItemKind::LedgerLine), 2);
    }

    #[test]
    fn page_count_and_empty_pages_are_stable() {
        let spec = PageSpec::default();
        let atoms: Vec<_> = (0..9).map(|i| atom(i % 8)).collect();
        assert_eq!(page_count(&atoms, &spec), 1);
        let many: Vec<_> = (0..200).map(|i| atom(i % 8)).collect();
        assert!(page_count(&many, &spec) >= 2);
        assert!(page(&atoms, &spec, 1).items.is_empty());
    }

    #[test]
    fn header_segments_and_first_note_do_not_overlap() {
        // Same spacing as the atoms stage; the test catches a regression of
        // the clef/key/time widths against the 4.7.4 style defaults.
        let width = |name: &str| {
            let g = scoretk_smufl::glyph(name).unwrap();
            units_to_fixed(g.bbox[2] - g.bbox[0])
        };
        let clef_x = CLEF_LEFT_MARGIN;
        let key_x = clef_x + width("gClef") + CLEF_KEY_RIGHT_MARGIN;
        let time_x =
            key_x + 4 * (width("accidentalSharp") + KEYSIG_ACCIDENTAL_DISTANCE) + KEY_TIME_DISTANCE;
        let note_x = time_x + width("timeSig4") + BAR_NOTE_DISTANCE;
        let atom = MeasureAtom {
            number: 0,
            width: note_x + SEGMENT_WIDTH,
            spring: 1024,
            events: vec![
                Atom::simple(clef_x, AtomKind::Clef { name: "gClef" }),
                Atom::simple(
                    key_x,
                    AtomKind::KeySig {
                        fifths: 4,
                        prev_fifths: 4,
                    },
                ),
                Atom::simple(
                    time_x,
                    AtomKind::TimeSig {
                        numerator: 4,
                        denominator: 4,
                    },
                ),
                Atom {
                    x: note_x,
                    kind: AtomKind::Chord,
                    chord: Some(ChordAtom {
                        duration: 4,
                        dots: 0,
                        grace: false,
                        stem_up: true,
                        stem_len: 3072,
                        notes: vec![NoteAtom {
                            line: 4,
                            head: "noteheadBlack",
                            accidental: None,
                        }],
                    }),
                    voice: 0,
                    measure: 0,
                    beams: 0,
                    beam_mode: scoretk_core::model::BeamMode::Auto,
                    beam_start: false,
                    beam_dir: None,
                    grace: false,
                    ..Default::default()
                },
            ],
            start_bar: crate::atoms::BarCode(1),
            end_bar: crate::atoms::BarCode(1),
            line_break: false,
            clef_name: "gClef",
            key_fifths: 0,
            mm_rest_count: 1,
            ..Default::default()
        };
        let list = page(&[atom], &PageSpec::default(), 0);
        let header: Vec<_> = list
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    ItemKind::Clef | ItemKind::Accidental | ItemKind::TimeSig | ItemKind::Notehead
                )
            })
            .collect();
        for (index, left) in header.iter().enumerate() {
            for right in header.iter().skip(index + 1) {
                let (ax0, ay0, ax1, ay1) = left.bbox(&list).unwrap();
                let (bx0, by0, bx1, by1) = right.bbox(&list).unwrap();
                let overlap_x = ax1.min(bx1) - ax0.max(bx0);
                let overlap_y = ay1.min(by1) - ay0.max(by0);
                assert!(
                    overlap_x <= 0 || overlap_y <= 0,
                    "{:?} and {:?} overlap by {overlap_x},{overlap_y}",
                    left.kind,
                    right.kind
                );
            }
        }
    }

    #[test]
    fn accidental_bbox_is_left_of_its_notehead() {
        // `accidentalslayout.cpp:286-305`: the accidental must clear the head
        // by `accidentalDistance` (0.25 sp). A regression here used to put the
        // sharp 3.5 sp to the left, i.e. after the preceding notehead.
        let atom = MeasureAtom {
            number: 0,
            width: 8 * SEGMENT_WIDTH,
            spring: 1024,
            events: vec![Atom {
                x: 4 * SP,
                kind: AtomKind::Chord,
                chord: Some(ChordAtom {
                    duration: 4,
                    dots: 0,
                    grace: false,
                    stem_up: true,
                    stem_len: 3072,
                    notes: vec![NoteAtom {
                        line: 4,
                        head: "noteheadBlack",
                        accidental: Some("accidentalSharp"),
                    }],
                }),
                voice: 0,
                measure: 0,
                beams: 0,
                beam_mode: scoretk_core::model::BeamMode::Auto,
                beam_start: false,
                beam_dir: None,
                grace: false,
                ..Default::default()
            }],
            start_bar: crate::atoms::BarCode(1),
            end_bar: crate::atoms::BarCode(1),
            line_break: false,
            clef_name: "gClef",
            key_fifths: 0,
            mm_rest_count: 1,
            ..Default::default()
        };
        let list = page(&[atom], &PageSpec::default(), 0);
        let accidental = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Accidental)
            .unwrap();
        let head = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Notehead)
            .unwrap();
        let (ax0, _, ax1, _) = accidental.bbox(&list).unwrap();
        let (hx0, _, hx1, _) = head.bbox(&list).unwrap();
        assert!(
            ax1 <= hx0,
            "accidental bbox {ax0}..{ax1} overlaps head {hx0}..{hx1}"
        );
        assert_eq!(hx0 - ax1, ACCIDENTAL_DISTANCE);
        // The accidental is emitted before its head in the display list.
        let accidental_index = list
            .items
            .iter()
            .position(|item| item.kind == ItemKind::Accidental)
            .unwrap();
        let head_index = list
            .items
            .iter()
            .position(|item| item.kind == ItemKind::Notehead)
            .unwrap();
        assert!(accidental_index < head_index);
    }

    #[test]
    fn an_explicit_beam_extends_both_stems_to_its_centre() {
        // `beamlayout.cpp:1262-1307`: a beam anchored by `<Beam>` extends
        // each stem to the beam line; the beam is drawn around that line
        // (`tdraw.cpp:920-936`) with `beamWidth` thickness.
        let mut first = atom(6);
        let mut second = atom(6);
        first.events[0].beams = 1;
        first.events[0].beam_start = true;
        second.events[0].beams = 1;
        first.events[0].x = 4 * SP;
        second.events[0].x = 12 * SP;
        let mut score_atoms = vec![first, second];
        score_atoms[1].events[0].x = 12 * SP;
        let list = page(&score_atoms, &PageSpec::default(), 0);
        assert_eq!(list.count(ItemKind::Beam), 1);
        let beam = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Beam)
            .unwrap();
        let Geometry::Beam { y1, y2, width, .. } = beam.geometry else {
            panic!("beam must be a slanted rectangle");
        };
        assert_eq!(width, BEAM_WIDTH);
        // The notes are level, so the beam is horizontal and sits above the
        // staff (up stem) with its centre on the stem tips.
        assert_eq!(y1, y2);
        let stems: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Stem)
            .collect();
        assert_eq!(stems.len(), 2);
        for stem in stems {
            let Geometry::Rect { y, h, .. } = stem.geometry else {
                panic!("stem must be a rectangle");
            };
            assert_eq!(y, y1 + BEAM_WIDTH / 2);
            assert!(h > 0);
        }
    }

    #[test]
    fn every_beamed_stem_end_lies_on_its_beam_line() {
        // Geometric guard for `Beam-CrossM1`: `tdraw.cpp:920-936` draws the
        // slab symmetrically around the beam segment, and
        // `beamlayout.cpp:1262-1307` extends every stem to that segment. A
        // stem that stops short (or overshoots) used to be invisible at page
        // scale; here the far end must sit on the interpolated centre line.
        let lines = [6, 5, 4, 5, 3, 5, 3, 4, 6];
        let mut atoms: Vec<_> = lines.iter().map(|&line| atom(line)).collect();
        for (index, measure) in atoms.iter_mut().enumerate() {
            measure.events[0].beams = 1;
            measure.events[0].beam_dir = Some(StemDirection::Up);
            if index == 0 || index == 5 {
                measure.events[0].beam_start = true;
            }
        }
        // One wide system: every stem on the page belongs to one of the two
        // groups, so the x-interval test cannot pick up an unrelated stem.
        let spec = PageSpec {
            printable_width: 1000 * SP,
            ..PageSpec::default()
        };
        let list = page(&atoms, &spec, 0);
        let beams: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Beam)
            .collect();
        assert_eq!(beams.len(), 2, "one slab per explicit beam group");
        let stems: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Stem)
            .collect();
        assert_eq!(stems.len(), lines.len());
        for beam in beams {
            let Geometry::Beam {
                x1,
                y1,
                x2,
                y2,
                width,
            } = beam.geometry
            else {
                panic!("beam must be a slanted rectangle");
            };
            let mut ends = 0;
            let mut lo = i32::MAX;
            let mut hi = i32::MIN;
            for stem in &stems {
                let Geometry::Rect { x, y, w, h } = stem.geometry else {
                    panic!("stem must be a rectangle");
                };
                let centre = x + w / 2;
                if centre < x1 - 64 || centre > x2 + 64 {
                    continue;
                }
                ends += 1;
                lo = lo.min(centre);
                hi = hi.max(centre);
                let expected = y1
                    + width / 2
                    + (i64::from(y2 - y1) * i64::from(centre - x1) / i64::from(x2 - x1)) as i32;
                // Up stems end at the top of the rectangle; down stems at
                // its bottom. Tolerance is 1/16 sp for integer rounding.
                let far_end = y.min(y + h);
                assert!(
                    (far_end - expected).abs() <= 64,
                    "stem far end {far_end} is off the beam line {expected}"
                );
            }
            assert!(ends >= 2, "a beam must cover at least two stems");
            // The slab starts and ends on the outermost stems (within half a
            // stem width), not on their notehead origins.
            assert!((x1 - lo).abs() <= 64, "beam start {x1} vs stem {lo}");
            assert!((x2 - hi).abs() <= 64, "beam end {x2} vs stem {hi}");
        }
    }

    #[test]
    fn an_explicit_down_beam_extends_its_stems_below_the_heads() {
        // `Beam::layout1` (`beamlayout.cpp:169-199`) applies one direction to
        // the whole group. A `<Beam><StemDirection>down</StemDirection>` on
        // high notes used to anchor the slab *above* the heads while the
        // stems were drawn down: `far_end - note_y` was clamped to zero and
        // every stem disappeared at page scale (`moonlight`, 234 stems).
        let mut first = atom(0);
        let mut second = atom(1);
        for measure in [&mut first, &mut second] {
            measure.events[0].beams = 1;
            measure.events[0].beam_dir = Some(StemDirection::Down);
        }
        first.events[0].beam_start = true;
        let list = page(&[first, second], &PageSpec::default(), 0);
        assert_eq!(list.count(ItemKind::Beam), 1);
        let beam = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Beam)
            .unwrap();
        let Geometry::Beam { y1, y2, .. } = beam.geometry else {
            panic!("beam must be a slanted rectangle");
        };
        assert!(y1 > 0 && y2 > 0, "a down beam sits below the top line");
        let stems: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Stem)
            .collect();
        assert_eq!(stems.len(), 2);
        for stem in stems {
            let Geometry::Rect { y, h, .. } = stem.geometry else {
                panic!("stem must be a rectangle");
            };
            assert!(h > 0, "an explicit down beam must keep visible stems");
            assert!(y >= 0, "a down stem starts at (or below) the head");
        }
    }

    #[test]
    fn a_beam_follows_its_chord_when_two_voices_do_not_overlap() {
        // `Measure::hasVoices(staffIdx, tick, len)` (`measure.cpp:2360-2419`,
        // used at `chordlayout.cpp:1584`) forces track parity only while two
        // voices sound together. A low chord of a non-overlapping second
        // voice keeps its automatic up stem in the atoms; the beam must use
        // that direction too (`beamlayout.cpp:169-199`), otherwise its slab
        // is anchored above the heads after a parity-forced down direction
        // and every stem collapses to zero length.
        let voice_measure = |voice: u32| {
            let mut measure = atom(8);
            measure.events[0].beams = 1;
            measure.events[0].beam_start = true;
            measure.events[0].voice = voice;
            let mut second = measure.events[0].clone();
            second.x = 12 * SP;
            second.beam_start = false;
            measure.events.push(second);
            measure.width = 20 * SP;
            measure
        };
        let atoms = [voice_measure(0), voice_measure(1)];
        let spec = PageSpec {
            printable_width: 1000 * SP,
            ..PageSpec::default()
        };
        let list = page(&atoms, &spec, 0);
        assert_eq!(list.count(ItemKind::Beam), 2);
        let stems: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Stem)
            .collect();
        assert_eq!(stems.len(), 4);
        for stem in stems {
            let Geometry::Rect { y, h, .. } = stem.geometry else {
                panic!("stem must be a rectangle");
            };
            assert!(h > 0, "both voices must keep visible stems");
            assert!(y < spec.margin_top + 8 * HALF_SP, "up stems start above the head");
        }
    }

    #[test]
    fn vertical_geometry_uses_fixed_point_staff_spaces() {
        let spec = PageSpec::default();
        let list = page(&[atom(8)], &spec, 0);
        let head = list
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Notehead)
            .unwrap();
        let (_, y) = head.origin();
        assert_eq!(y, spec.margin_top + 8 * HALF_SP);
    }
}
