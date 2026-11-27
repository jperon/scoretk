// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port of `src/engraving/rendering/score/chordlayout.cpp:1536-1643`
// (`ChordLayout::computeUp`, `computeAutoStemDirection`) and
// `stemlayout.cpp:42-160` (`StemLayout::calcDefaultStemLength`) of MuseScore
// Studio 4.7.4 (tag v4.7.4), Copyright (C) MuseScore Limited and others,
// modified (rewritten in Rust) in 2026. Only plain numbers are kept: no
// `EngravingItem` graph, no DOM mutation.
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

//! Compact per-measure atoms, computed once in `Engraver::new`.
//!
//! An atom stores x positions and the numbers needed by page layout; it never
//! borrows the `Score`. This is stage 1 of the plan (§4) with fixed
//! proportional spacing: the real `HorizontalSpacing` springs arrive in P2.

use alloc::vec::Vec;

use scoretk_core::model::{
    AccidentalType, BarLineType, ChordRestKind, ClefType, DurationType, NoteHeadType, Score,
    SegmentItem, SegmentType, StemDirection, Transpose,
};

use scoretk_smufl::{glyph, units_to_fixed};

use crate::style::{
    CLEF_KEY_RIGHT_MARGIN, CLEF_LEFT_MARGIN, KEYSIG_ACCIDENTAL_DISTANCE, KEY_TIME_DISTANCE,
    MIN_NOTE_DISTANCE, QUARTER_NOTE_SPACE, SEGMENT_WIDTH, SP, SYSTEM_HEADER_MIN_START,
};

/// Width of a SMuFL outline in fixed staff spaces (em = 4 sp).
fn glyph_width(name: &str) -> i32 {
    glyph(name)
        .map(|g| units_to_fixed(g.bbox[2] - g.bbox[0]))
        .unwrap_or(1024)
}

/// A note inside a chord atom: position in half-spaces (top line = 0) and
/// the SMuFL head name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteAtom {
    pub line: i32,
    pub head: &'static str,
    pub accidental: Option<&'static str>,
}

/// A chord with everything the page pass needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordAtom {
    pub duration: u8,
    pub dots: u8,
    pub stem_up: bool,
    /// Stem length in 1/1024 staff space, already computed from the C++ rule.
    pub stem_len: i32,
    pub notes: Vec<NoteAtom>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomKind {
    Clef { name: &'static str },
    KeySig { fifths: i8 },
    TimeSig { numerator: u8, denominator: u8 },
    Chord,
    Rest { duration: u8, dots: u8 },
}

/// One event of a measure atom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    /// x offset from the beginning of the measure, in fixed staff spaces.
    pub x: i32,
    pub kind: AtomKind,
    pub chord: Option<ChordAtom>,
}

impl Atom {
    pub(crate) fn simple(x: i32, kind: AtomKind) -> Self {
        Self {
            x,
            kind,
            chord: None,
        }
    }
}

/// Bar-line code: 0 none, 1 single, 2 double, 3 final.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarCode(pub u8);

/// One measure, reduced to numbers and glyph names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureAtom {
    pub number: usize,
    /// Minimum width from the natural spring lengths of `HorizontalSpacing`.
    pub width: i32,
    /// Sum of the segments' duration stretches (1/1024); each segment's
    /// spring constant is `1 / stretch` (`horizontalspacing.cpp:204`).
    pub spring: i32,
    pub events: Vec<Atom>,
    pub start_bar: BarCode,
    pub end_bar: BarCode,
    /// A `<LayoutBreak>` line ends the system after this measure.
    pub line_break: bool,
    /// Clef in force at this measure, repeated as a system header when the
    /// measure opens a system (`measurelayout.cpp:2530-2590`, `genClef`).
    pub clef_name: &'static str,
    /// Key signature in force at this measure, repeated as a system header
    /// when non-C (`measurelayout.cpp:2597-2660`, `genKeysig`).
    pub key_fifths: i8,
}

impl MeasureAtom {
    /// Returns a copy preceded by the clef/key header MuseScore generates at
    /// the start of a system. // Port of `measurelayout.cpp:2530-2590` and
    /// `:2597-2660`: `genClef` and `genKeysig` are true by default, so a
    /// clef (and a non-C key) is generated on every system, not only the
    /// first one. The header has fixed width and does not take part in the
    /// spring justification.
    pub fn with_system_header(&self) -> Self {
        let mut atom = self.clone();
        let clef_width = glyph_width(self.clef_name);
        let mut header_width = CLEF_LEFT_MARGIN + clef_width + CLEF_KEY_RIGHT_MARGIN + SP / 10;
        let mut events = Vec::with_capacity(self.events.len() + 2);
        events.push(Atom::simple(
            CLEF_LEFT_MARGIN,
            AtomKind::Clef {
                name: self.clef_name,
            },
        ));
        if self.key_fifths != 0 {
            let name = if self.key_fifths > 0 {
                "accidentalSharp"
            } else {
                "accidentalFlat"
            };
            let key_x = CLEF_LEFT_MARGIN + clef_width + CLEF_KEY_RIGHT_MARGIN + SP / 10;
            events.push(Atom::simple(
                key_x,
                AtomKind::KeySig {
                    fifths: self.key_fifths,
                },
            ));
            header_width += self.key_fifths.unsigned_abs().min(7) as i32
                * (glyph_width(name) + KEYSIG_ACCIDENTAL_DISTANCE)
                + KEY_TIME_DISTANCE;
        }
        for event in &mut atom.events {
            event.x += header_width;
        }
        atom.width += header_width;
        events.append(&mut atom.events);
        atom.events = events;
        atom
    }
}

/// Diatonic index of a note, with C = 0 in each octave.
fn diatonic(tpc_value: i8, pitch: u8) -> i32 {
    let step = (i32::from(tpc_value) - 13).rem_euclid(7); // F,C,G,D,A,E,B
    let alteration = (i32::from(tpc_value) - 13).div_euclid(7);
    const STEP_PC: [i32; 7] = [5, 0, 7, 2, 9, 4, 11];
    let pc = STEP_PC[step as usize];
    // C,D,E,F,G,A,B order: 0,1,2,3,4,5,6.
    const ORDER: [i32; 7] = [3, 0, 4, 1, 5, 2, 6];
    let order = ORDER[step as usize];
    let octave = (i32::from(pitch) - pc - alteration).div_euclid(12) - 1;
    octave * 7 + order
}

/// Diatonic index of the bottom staff line for a clef.
///
/// // Port of `engraving/dom/clef.cpp` (`ClefType::line`, 1..5 from bottom):
/// the reference note sits on `line`, so the bottom line is two diatonic
/// steps lower for each staff line below it.
fn bottom_line_note(clef: ClefType) -> i32 {
    let (reference, line) = match clef {
        ClefType::G
        | ClefType::G8Vb
        | ClefType::G8Va
        | ClefType::G15Mb
        | ClefType::G15Ma
        | ClefType::G1 => (4 * 7 + 4, 2), // G4 on line 2
        ClefType::F
        | ClefType::F8Vb
        | ClefType::F8Va
        | ClefType::F15Mb
        | ClefType::F15Ma
        | ClefType::FBaritone
        | ClefType::FSubbass => (3 * 7 + 3, 4), // F3 on line 4
        ClefType::C1 => (4 * 7, 1),
        ClefType::C2 => (4 * 7, 2),
        ClefType::C3 => (4 * 7, 3),
        ClefType::C4 => (4 * 7, 4),
        ClefType::C5 => (4 * 7, 5),
        _ => (4 * 7 + 2, 1), // default: E4 on the bottom line
    };
    reference - 2 * (line - 1)
}

/// Note position in half-space units: top line = 0, bottom line = 8.
fn note_line(tpc_value: i8, pitch: u8, bottom: i32) -> i32 {
    8 - (diatonic(tpc_value, pitch) - bottom)
}

fn clef_name(clef: ClefType) -> &'static str {
    match clef {
        ClefType::G => "gClef",
        ClefType::G8Vb => "gClef8vb",
        ClefType::G8Va => "gClef8va",
        ClefType::G15Mb => "gClef15mb",
        ClefType::G15Ma => "gClef15ma",
        ClefType::G1 => "gClef",
        ClefType::F => "fClef",
        ClefType::F8Vb => "fClef8vb",
        ClefType::F8Va => "fClef8va",
        ClefType::F15Mb => "fClef15mb",
        ClefType::F15Ma => "fClef15ma",
        ClefType::FBaritone | ClefType::FSubbass => "fClef",
        ClefType::C1 | ClefType::C2 | ClefType::C3 | ClefType::C4 | ClefType::C5 => "cClef",
        _ => "gClef",
    }
}

fn head_name(head: NoteHeadType, duration: DurationType) -> &'static str {
    let effective = if head == NoteHeadType::Auto {
        duration.head_type()
    } else {
        head
    };
    match effective {
        NoteHeadType::Breve => "noteheadDoubleWhole",
        NoteHeadType::Whole => "noteheadWhole",
        NoteHeadType::Half => "noteheadHalf",
        _ => "noteheadBlack",
    }
}

fn accidental_name(kind: AccidentalType) -> Option<&'static str> {
    match kind {
        AccidentalType::Flat => Some("accidentalFlat"),
        AccidentalType::Natural => Some("accidentalNatural"),
        AccidentalType::Sharp => Some("accidentalSharp"),
        AccidentalType::DoubleFlat => Some("accidentalDoubleFlat"),
        AccidentalType::DoubleSharp => Some("accidentalDoubleSharp"),
        _ => None,
    }
}

pub fn duration_code(duration: DurationType) -> u8 {
    match duration {
        DurationType::Long => 0,
        DurationType::Breve => 1,
        DurationType::Whole | DurationType::Measure => 2,
        DurationType::Half => 3,
        DurationType::Quarter => 4,
        DurationType::Eighth => 5,
        DurationType::Sixteenth => 6,
        DurationType::ThirtySecond => 7,
        DurationType::SixtyFourth => 8,
        DurationType::OneHundredTwentyEighth => 9,
        DurationType::TwoHundredFiftySixth => 10,
        DurationType::FiveHundredTwelfth => 11,
        DurationType::OneThousandTwentyFourth => 12,
        DurationType::Zero | DurationType::Invalid => 4,
    }
}

pub fn rest_name(duration: u8) -> &'static str {
    match duration {
        0 => "restMaxima",
        1 => "restLonga",
        2 => "restWhole",
        3 => "restHalf",
        5 => "rest8th",
        6 => "rest16th",
        7 => "rest32nd",
        8 => "rest64th",
        9 => "rest128th",
        10 => "rest256th",
        11 => "rest512th",
        12 => "rest1024th",
        _ => "restQuarter",
    }
}

/// True for the duration codes that carry a stem (whole and breve do not).
pub fn has_stem(duration: u8) -> bool {
    duration >= 3
}

/// // Port of `horizontalspacing.cpp:805-812` (`durationStretchForTicks`):
/// `measureSpacing^log2(ticks / quarter)` with the 4.7.4 default slope 1.5.
/// The values below are that formula in 1/1024, precomputed for power-of-two
/// durations; `dots` multiplies the ratio by `2 - 2^-dots` (`:206-215` of
/// `durationtype.cpp`). No floating point is needed in the `no_std` crate.
/// // Port of `horizontalspacing.cpp:773-803`
/// (`durationStretchForMMRests`): a collapsed group is as wide as a
/// `timeSig` rest plus up to `mmRestMaxWidthIncrease` (8) extra
/// `1/denominator` steps, i.e. `measureSpacing^log2(ratio)` in quarters.
pub fn mmrest_stretch(count: u16, denominator: i64) -> i32 {
    const RATIO_STRETCH: [i32; 13] = [
        0, 1024, 1536, 1947, 2304, 2688, 3020, 3326, 3456, 3571, 3678, 3778, 3871,
    ];
    let denominator = i32::try_from(denominator).unwrap_or(4);
    let quarters = if denominator > 0 { 4 / denominator } else { 1 };
    let extra = (count.saturating_sub(2) as i32).min(8).max(0) * quarters.max(1);
    let ratio = 4 * quarters.max(1) + extra;
    RATIO_STRETCH.get(ratio as usize).copied().unwrap_or(3871)
}

pub fn duration_stretch(duration: u8, dots: u8) -> i32 {
    const BASE: [i32; 13] = [
        3456, 2304, 2304, 1536, 1024, 683, 455, 303, 202, 135, 90, 60, 40,
    ];
    const DOT: [i32; 4] = [1024, 1298, 1421, 1479];
    let base = BASE.get(usize::from(duration)).copied().unwrap_or(1024);
    let factor = DOT[usize::from(dots.min(3))];
    (base * factor) / 1024
}

pub fn flag_name(duration: u8, up: bool) -> Option<&'static str> {
    match (duration, up) {
        (5, true) => Some("flag8thUp"),
        (5, false) => Some("flag8thDown"),
        (6, true) => Some("flag16thUp"),
        (6, false) => Some("flag16thDown"),
        (7, true) => Some("flag32ndUp"),
        (7, false) => Some("flag32ndDown"),
        (8, true) => Some("flag64thUp"),
        (8, false) => Some("flag64thDown"),
        (9, true) => Some("flag128thUp"),
        (9, false) => Some("flag128thDown"),
        _ => None,
    }
}

/// // Port of `chordlayout.cpp:1626-1643` (`computeAutoStemDirection`):
/// pair the outermost notes around the middle line, sum the two distances,
/// and take the first non-zero sum (positive means up).
///
/// `noteDistances` is `note->line() - centerLine` (`chord.cpp`, via
/// `stemlayout.cpp:71` where the middle line is `(staffLines - 1) * 2`).
fn auto_stem_up(lines: &[i32]) -> bool {
    if lines.is_empty() {
        return true;
    }
    let center = 4; // (5 lines - 1) * 2 / 2 : middle line in half-spaces
    let mut left = 0usize;
    let mut right = lines.len().saturating_sub(1);
    while left <= right {
        let sum = (lines[left] - center) + (lines[right] - center);
        if sum != 0 {
            return sum > 0; // higher notes (smaller line) get down stems
        }
        if left == right {
            break;
        }
        left += 1;
        right -= 1;
    }
    true
}

/// // Port of `chordlayout.cpp:1564-1588`: an explicit stem direction wins
/// when there is no beam (P1 has no beams). With two voices on the staff the
/// stems are forced by track parity (`computeUp`: `track() % 2 == 0`);
/// otherwise the automatic rule applies.
fn stem_up(lines: &[i32], explicit: StemDirection, multi_voice: bool, track: u32) -> bool {
    match explicit {
        StemDirection::Up => true,
        StemDirection::Down => false,
        StemDirection::Auto => {
            if multi_voice {
                track.is_multiple_of(2)
            } else {
                auto_stem_up(lines)
            }
        }
    }
}

/// // Port of `stemlayout.cpp:42-160` (`StemLayout::calcDefaultStemLength`):
/// all internal arithmetic is in quarter spaces; with line distance 1 and
/// magnitude 1, `defaultStemLength = stemLength * 4 = 14`, `quarterSpacesPerLine
/// = floor(lineDistance * 2) = 2`, `chordHeight = (downLine - upLine) * 2`,
/// `shortestStem = 2.5 * 4 = 10` and the final length is
/// `chordHeight / 4 * sp + stemLength / 4 * sp`, to which the middle-line
/// extension of `:122-159` is added. Beams and tremolos are P3.
/// Hooks and stem shortening follow `stemlayout.cpp:326-397` below.
fn stem_length(lines: &[i32], up: bool, duration: u8) -> i32 {
    if lines.is_empty() {
        return 3584;
    }
    let top = *lines.iter().min().unwrap();
    let bottom = *lines.iter().max().unwrap();
    let chord_height_qs = (bottom - top) * 2;
    let default_qs = 14;
    let mut stem_qs = default_qs;
    // // Port of `stemlayout.cpp:77, 345-397` (`maxReduction`, style
    // `shortenStem` true, `shortStemStartLocation` 1, traditional hooks):
    // a stem ending outside the staff is shortened by up to one staff space
    // according to the [extension][beamCount] table; traditional hooks cap
    // the reduction at one quarter space.
    let short_start = 3; // shortStemStartLocation 1 * quarterSpacesPerLine 2 + 1
    let down_short_start = (5 - 1) * (2 * 2) + short_start; // 19
    let end = if up {
        top * 2 - default_qs
    } else {
        bottom * 2 + default_qs
    };
    let outside = if up {
        if end <= -short_start {
            Some((-end - short_start).abs())
        } else {
            None
        }
    } else if end >= down_short_start {
        Some((end - down_short_start).abs())
    } else {
        None
    };
    if let Some(extension) = outside {
        const REDUCTIONS: [i32; 5] = [1, 2, 3, 4, 4];
        let half_spaces = (extension / 2).min(4) as usize;
        let mut reduction = REDUCTIONS[half_spaces];
        if duration >= 5 {
            // Traditional flag (`useStraightNoteFlags` false, styledef.cpp:216).
            reduction = reduction.min(1);
        }
        stem_qs = (stem_qs - reduction).max(10);
    }
    let mut final_qs = chord_height_qs + stem_qs;
    // // Port of `stemlayout.cpp:122-159`: a normal-sized chord's stem is
    // extended until its far end reaches the middle staff line (two spaces
    // from the top line, i.e. 8 quarter spaces). Without this step a stem
    // starting on a low note with an up direction stops short of the middle
    // line; MuseScore adds the missing length to `finalStemLength`.
    let start = if up { bottom * 2 } else { top * 2 };
    let stem_end = if up {
        start - final_qs
    } else {
        start + final_qs
    };
    let middle_line_qs = 8; // minStaffOverlap(false, 5, 0, …) with wide beams off
    let extra_qs = if up {
        (stem_end - middle_line_qs).max(0)
    } else {
        (middle_line_qs - stem_end).max(0)
    };
    final_qs += extra_qs;
    final_qs * 256 // quarter spaces in 1/1024 staff-space units
}

fn bar_code(kind: BarLineType) -> BarCode {
    match kind {
        BarLineType::Normal | BarLineType::Tick | BarLineType::Short => BarCode(1),
        BarLineType::Double
        | BarLineType::StartRepeat
        | BarLineType::EndRepeat
        | BarLineType::EndStartRepeat
        | BarLineType::DoubleShort
        | BarLineType::Dotted
        | BarLineType::DoubleDotted
        | BarLineType::Broken
        | BarLineType::Dashed => BarCode(2),
        BarLineType::End => BarCode(3),
    }
}

/// Computes all measure atoms of a score (single staff only).
pub fn compute(score: &Score) -> Vec<MeasureAtom> {
    let staff = &score.staves[0];
    let mut clef = staff.default_clef.concert;
    if clef == ClefType::Invalid {
        clef = ClefType::G;
    }
    let mut bottom = bottom_line_note(clef);
    let mut key_fifths = 0i8;
    // MuseScore engraves the written pitch by default (`concertPitch` false):
    // transpose the concert pitch of the pivot model by the inverse of the
    // instrument's transposition (`engraving/dom/transpose.cpp`).
    let transpose = score
        .parts
        .get(staff.part.index())
        .and_then(|part| part.instrument.transpose);
    let inverse = transpose.map(|value| Transpose::new(-value.diatonic, -value.chromatic));
    // // Port of `horizontalspacing.cpp:1224` and `:1193-1197`: the minimum
    // shape padding is 0.1 sp and the distance from a bar line to the first
    // chord is `barNoteDistance` (the style value, e.g. 0.6 sp in 3.x files
    // with an explicit `<barNoteDistance>`).
    let style = &score.engraving.style;
    let bar_note_distance = (style.bar_note_distance_sp * SP as f64) as i32 + SP / 10;
    let time_sig_distance = (style.system_header_time_sig_distance_sp * SP as f64) as i32;
    let mut atoms = Vec::with_capacity(score.measures.len());

    let mut measure_index = 0;
    while let Some(measure) = score.measures.get(measure_index) {
        // A `<multiMeasureRest>N</multiMeasureRest>` folds N source measures
        // into one displayed measure (`measurelayout.cpp:194-524`).
        let mm_count = measure.multi_measure_rest.unwrap_or(1).max(1) as usize;
        // A line break carried by a folded measure still ends the displayed
        // system after the multi-measure rest.
        let mut line_break = measure.line_break;
        for skipped in &score.measures[measure_index + 1..(measure_index + mm_count).min(score.measures.len())] {
            line_break |= skipped.line_break;
        }
        measure_index += mm_count;
        let mm_stretch = if mm_count > 1 {
            mmrest_stretch(mm_count as u16, measure.nominal_len.denominator())
        } else {
            0
        };
        let first = atoms.is_empty();
        let mut x = if first {
            bar_note_distance.max(SYSTEM_HEADER_MIN_START)
        } else {
            bar_note_distance
        };
        let mut events = Vec::new();
        let mut spring = 0;
        let mut has_clef = false;
        let mut has_key = false;
        let mut has_time = false;
        if first {
            // // Port of `horizontalspacing.cpp:1199-1207`
            // (`getFirstSegmentXPos`): the header clef starts at
            // `clefLeftMargin`; its width and `clefKeyRightMargin` determine
            // where the key signature segment can start.
            let name = clef_name(clef);
            let clef_x = CLEF_LEFT_MARGIN;
            events.push(Atom::simple(clef_x, AtomKind::Clef { name }));
            x = clef_x + glyph_width(name) + CLEF_KEY_RIGHT_MARGIN + SP / 10;
            has_clef = true;
        }
        // // Port of `chordlayout.cpp:1584` (`measure()->hasVoices`): once a
        // measure contains two voices, the stems of every voice in it are
        // forced by track parity, not only the chords sharing a segment.
        let multi_voice = measure.segments.iter().any(|segment| {
            segment
                .iter_elements()
                .filter(|(_, item)| matches!(item, SegmentItem::ChordRest(_)))
                .count()
                > 1
        });

        for segment in &measure.segments {
            if segment.kind == SegmentType::ChordRest {
                // P1: fixed proportional spacing, one x per chord/rest segment
                // (plan §4, stage 1; P2 replaces this with real springs). All
                // voices of a segment share that x; MuseScore forces their
                // stem directions by track parity (`chordlayout.cpp:1584`).
                let chord_rests: Vec<(u32, &_)> = segment
                    .iter_elements()
                    .filter_map(|(track, item)| match item {
                        SegmentItem::ChordRest(cr) => Some((track.raw(), cr)),
                        _ => None,
                    })
                    .collect();
                if chord_rests.is_empty() {
                    x += SEGMENT_WIDTH;
                    continue;
                }
                // // Port of `horizontalspacing.cpp:730-745`: the natural
                // width of a chord/rest segment is `3.5 sp * stretch`, where
                // stretch comes from its duration; `minNoteDistance` and the
                // notehead/accidental/dot widths give the lower bound.
                let mut segment_width = SEGMENT_WIDTH;
                for (track, cr) in chord_rests {
                    let duration = duration_code(cr.duration.kind);
                    let stretch = if mm_stretch > 0 && matches!(cr.kind, ChordRestKind::Rest(_)) {
                        mm_stretch
                    } else {
                        duration_stretch(duration, cr.duration.dots)
                    };
                    spring += stretch;
                    let mut min_width = glyph_width("noteheadBlack") + MIN_NOTE_DISTANCE;
                    if cr.duration.dots > 0 {
                        min_width += cr.duration.dots as i32 * glyph_width("augmentationDot");
                    }
                    segment_width = (QUARTER_NOTE_SPACE * stretch / 1024).max(min_width);
                    match &cr.kind {
                        ChordRestKind::Chord(chord) => {
                            let mut notes = Vec::new();
                            for id in &chord.notes {
                                if let Some(note) = score.notes.get(id.index()) {
                                    if !note.visible {
                                        continue;
                                    }
                                    let head = head_name(note.head, cr.duration.kind);
                                    let accidental = note
                                        .accidental
                                        .filter(|a| a.visible)
                                        .and_then(|a| accidental_name(a.kind));
                                    let (written_tpc, written_pitch) = match inverse {
                                        Some(interval) => {
                                            let _ = interval.diatonic;
                                            // The pitch class keeps its spelling for
                                            // octave transpositions; other intervals
                                            // are P3 (accidental/stack work).
                                            (
                                                note.tpc,
                                                (i32::from(note.pitch) + interval.chromatic)
                                                    .clamp(0, 127)
                                                    as u8,
                                            )
                                        }
                                        None => (note.tpc, note.pitch),
                                    };
                                    notes.push(NoteAtom {
                                        line: note_line(written_tpc, written_pitch, bottom),
                                        head,
                                        accidental,
                                    });
                                }
                            }
                            notes.sort_by_key(|note| note.line);
                            let lines: Vec<i32> = notes.iter().map(|n| n.line).collect();
                            let up = stem_up(&lines, chord.engraving.stem, multi_voice, track);
                            let chord_atom = ChordAtom {
                                duration,
                                dots: cr.duration.dots,
                                stem_up: up,
                                stem_len: stem_length(&lines, up, duration),
                                notes,
                            };
                            events.push(Atom {
                                x,
                                kind: AtomKind::Chord,
                                chord: Some(chord_atom),
                            });
                        }
                        ChordRestKind::Rest(_) => {
                            events.push(Atom::simple(
                                x,
                                AtomKind::Rest {
                                    duration,
                                    dots: cr.duration.dots,
                                },
                            ));
                        }
                    }
                }
                x += segment_width;
                continue;
            }

            for item in segment.elements().iter().flatten() {
                match item {
                    SegmentItem::Clef(new_clef) if segment.kind.is_clef_type() && !has_clef => {
                        clef = new_clef.concert;
                        bottom = bottom_line_note(clef);
                        let name = clef_name(clef);
                        let clef_x = x + CLEF_LEFT_MARGIN;
                        events.push(Atom::simple(clef_x, AtomKind::Clef { name }));
                        x = clef_x + glyph_width(name) + CLEF_KEY_RIGHT_MARGIN + SP / 10;
                        has_clef = true;
                    }
                    SegmentItem::KeySig(key) if segment.kind == SegmentType::KeySig && !has_key => {
                        let fifths = key.concert_key.fifths();
                        key_fifths = fifths;
                        if fifths != 0 {
                            events.push(Atom::simple(x, AtomKind::KeySig { fifths }));
                            // // Port of `styledef.cpp:241` (`keysigAccidentalDistance`):
                            // consecutive accidentals are one glyph width plus
                            // 0.3 sp apart; `keyTimesigDistance` follows them.
                            let accidental = if fifths > 0 {
                                "accidentalSharp"
                            } else {
                                "accidentalFlat"
                            };
                            let count = fifths.unsigned_abs().min(7) as i32;
                            x += count * (glyph_width(accidental) + KEYSIG_ACCIDENTAL_DISTANCE)
                                + KEY_TIME_DISTANCE;
                        }
                        has_key = true;
                    }
                    SegmentItem::TimeSig(time)
                        if segment.kind == SegmentType::TimeSig && !has_time =>
                    {
                        let numerator = u8::try_from(time.numerator).unwrap_or(4);
                        let denominator = u8::try_from(time.denominator).unwrap_or(4);
                        events.push(Atom::simple(
                            x,
                            AtomKind::TimeSig {
                                numerator,
                                denominator,
                            },
                        ));
                        // // Port of `horizontalspacing.cpp:1345-1353`: after
                        // a system header time signature the gap is
                        // `systemHeaderTimeSigDistance` (2 sp), not the
                        // ordinary `barNoteDistance`.
                        x += glyph_width("timeSig4") + time_sig_distance;
                        has_time = true;
                    }
                    _ => {}
                }
            }
        }

        let mut atom = MeasureAtom {
            number: measure.number,
            width: x,
            spring,
            events,
            start_bar: bar_code(measure.start_bar_line),
            end_bar: bar_code(measure.end_bar_line),
            line_break,
            clef_name: clef_name(clef),
            key_fifths,
        };
        // // Port of `systemlayout.cpp:402-407`
        // (`MeasureLayout::createEndBarLines`): MuseScore generates a final
        // barline on the last measure even when the source has none. Its
        // two strokes are drawn to the left of the measure's right edge, so
        // the system's last fixed width is `endBarWidth + endBarDistance`
        // (styledef.cpp:147,149).
        if score
            .measures
            .last()
            .is_some_and(|last| last.number == measure.number)
            && atom.end_bar.0 == 1
        {
            atom.end_bar = BarCode(3);
        }
        atoms.push(atom);
    }
    atoms
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::tpc;

    #[test]
    fn treble_note_lines_follow_the_staff() {
        // E4 is the bottom line (8), G4 the one above (6), F5 the top (0).
        let bottom = bottom_line_note(ClefType::G);
        assert_eq!(diatonic(tpc::E, 64), 30, "diatonic E4");
        assert_eq!(note_line(tpc::E, 64, bottom), 8);
        assert_eq!(note_line(tpc::G, 67, bottom), 6);
        assert_eq!(note_line(tpc::F, 77, bottom), 0);
        assert_eq!(note_line(tpc::C, 60, bottom), 10); // C4 needs a ledger line
    }

    #[test]
    fn bass_note_lines_follow_the_staff() {
        let bottom = bottom_line_note(ClefType::F);
        assert_eq!(note_line(tpc::G, 43, bottom), 8); // G2 bottom line
        assert_eq!(note_line(tpc::A, 57, bottom), 0); // A3 top line
    }

    #[test]
    fn auto_stem_direction_matches_musescore() {
        // A single high note gets a down stem, a low note an up stem.
        assert!(!auto_stem_up(&[0]));
        assert!(auto_stem_up(&[8]));
        // Symmetric chord: the middle line leads to the default "up".
        assert!(auto_stem_up(&[2, 6]));
        // Two voices: even tracks up, odd tracks down (`chordlayout.cpp:1586`).
        assert!(stem_up(&[0], StemDirection::Auto, true, 0));
        assert!(!stem_up(&[0], StemDirection::Auto, true, 1));
    }

    #[test]
    fn duration_stretch_follows_the_4_7_4_spacing_slope() {
        // `horizontalspacing.cpp:805-812`: 1.5^log2(ticks/quarter).
        assert_eq!(duration_stretch(4, 0), 1024); // quarter
        assert_eq!(duration_stretch(3, 0), 1536); // half
        assert_eq!(duration_stretch(5, 0), 683); // eighth
        assert_eq!(duration_stretch(2, 0), 2304); // whole
        assert_eq!(duration_stretch(4, 1), 1298); // dotted quarter
    }

    #[test]
    fn stem_length_is_at_least_shortest() {
        assert_eq!(stem_length(&[4], true, 4), 3072); // one stem shortening
        assert_eq!(stem_length(&[0], true, 4), 2560); // shortestStem
    }

    #[test]
    fn stem_reaches_the_middle_line_from_outside_the_staff() {
        // `stemlayout.cpp:122-159`: a stem starting 2.5 sp above the top
        // line is extended by 3.5 sp so that its far end reaches the middle
        // line. Without the extension it would stop 1.5 sp short of it.
        assert_eq!(stem_length(&[-10], false, 4), 28 * 256);
        // A stem starting 2 sp below the bottom line gains 0.5 sp.
        assert_eq!(stem_length(&[12], true, 4), 16 * 256);
        // A note inside the staff only gets the shortening rule: 3 sp.
        assert_eq!(stem_length(&[4], false, 4), 12 * 256);
    }
}
