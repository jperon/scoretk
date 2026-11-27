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
    AccidentalType, BarLineType, ChordRestKind, ClefType, DurationType, NoteHeadType,
    Score, SegmentItem, SegmentType, StemDirection, Transpose,
};

use crate::style::{FIRST_MEASURE_EXTRA, MEASURE_GAP, SEGMENT_WIDTH};

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
    fn simple(x: i32, kind: AtomKind) -> Self {
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
    pub width: i32,
    pub events: Vec<Atom>,
    pub start_bar: BarCode,
    pub end_bar: BarCode,
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
        ClefType::G | ClefType::G8Vb | ClefType::G8Va | ClefType::G15Mb
        | ClefType::G15Ma | ClefType::G1 => (4 * 7 + 4, 2), // G4 on line 2
        ClefType::F | ClefType::F8Vb | ClefType::F8Va | ClefType::F15Mb
        | ClefType::F15Ma | ClefType::FBaritone | ClefType::FSubbass => {
            (3 * 7 + 3, 4)
        } // F3 on line 4
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

/// // Port of `chordlayout.cpp:1564-1569`: an explicit stem direction wins
/// when there is no beam (P1 has no beams), otherwise the automatic rule
/// applies.
fn stem_up(lines: &[i32], explicit: StemDirection) -> bool {
    match explicit {
        StemDirection::Up => true,
        StemDirection::Down => false,
        StemDirection::Auto => auto_stem_up(lines),
    }
}

/// // Port of `stemlayout.cpp:60-71, 85-99, 122` (`calcDefaultStemLength`):
/// all internal arithmetic is in quarter spaces; with line distance 1 and
/// magnitude 1, `defaultStemLength = 3.5 sp * 4 = 14`, `quarterSpacesPerLine
/// = 2`, `chordHeight = (downLine - upLine) * 2`, and the final length is
/// `chordHeight / 4 * sp + stemLength / 4 * sp`. Beams and tremolos are P3.
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
    let short_start = 1 * 2 + 1; // 3 quarter spaces
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
    chord_height_qs * 256 + stem_qs * 256 // /4 sp, 1/1024 units
}

fn bar_code(kind: BarLineType) -> BarCode {
    match kind {
        BarLineType::Normal | BarLineType::Tick | BarLineType::Short => BarCode(1),
        BarLineType::Double | BarLineType::StartRepeat | BarLineType::EndRepeat
        | BarLineType::EndStartRepeat | BarLineType::DoubleShort | BarLineType::Dotted
        | BarLineType::DoubleDotted | BarLineType::Broken | BarLineType::Dashed => BarCode(2),
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
    // MuseScore engraves the written pitch by default (`concertPitch` false):
    // transpose the concert pitch of the pivot model by the inverse of the
    // instrument's transposition (`engraving/dom/transpose.cpp`).
    let transpose = score
        .parts
        .get(staff.part.index())
        .and_then(|part| part.instrument.transpose);
    let inverse = transpose.map(|value| Transpose::new(-value.diatonic, -value.chromatic));
    let mut atoms = Vec::with_capacity(score.measures.len());

    for measure in &score.measures {
        let first = atoms.is_empty();
        let mut x = if first { FIRST_MEASURE_EXTRA } else { MEASURE_GAP };
        let mut events = Vec::new();
        let mut has_clef = false;
        let mut has_key = false;
        let mut has_time = false;

        for segment in &measure.segments {
            if segment.kind == SegmentType::ChordRest {
                // P1: fixed proportional spacing, one x per chord/rest segment
                // (plan §4, stage 1; P2 replaces this with real springs).
                let item = segment.elements().iter().flatten().find_map(|item| match item {
                    SegmentItem::ChordRest(cr) => Some(cr),
                    _ => None,
                });
                let Some(cr) = item else {
                    x += SEGMENT_WIDTH;
                    continue;
                };
                let duration = duration_code(cr.duration.kind);
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
                                        (note.tpc,
                                         (i32::from(note.pitch) + interval.chromatic).clamp(0, 127) as u8)
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
                        let up = stem_up(&lines, chord.engraving.stem);
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
                x += SEGMENT_WIDTH;
                continue;
            }

            for item in segment.elements().iter().flatten() {
                match item {
                    SegmentItem::Clef(new_clef) if segment.kind.is_clef_type() && !has_clef => {
                        clef = new_clef.concert;
                        bottom = bottom_line_note(clef);
                        events.push(Atom::simple(x, AtomKind::Clef {
                            name: clef_name(clef),
                        }));
                        x += SEGMENT_WIDTH;
                        has_clef = true;
                    }
                    SegmentItem::KeySig(key) if segment.kind == SegmentType::KeySig && !has_key => {
                        events.push(Atom::simple(
                            x,
                            AtomKind::KeySig {
                                fifths: key.concert_key.fifths(),
                            },
                        ));
                        x += SEGMENT_WIDTH;
                        has_key = true;
                    }
                    SegmentItem::TimeSig(time) if segment.kind == SegmentType::TimeSig && !has_time => {
                        events.push(Atom::simple(
                            x,
                            AtomKind::TimeSig {
                                numerator: u8::try_from(time.numerator).unwrap_or(4),
                                denominator: u8::try_from(time.denominator).unwrap_or(4),
                            },
                        ));
                        x += SEGMENT_WIDTH;
                        has_time = true;
                    }
                    _ => {}
                }
            }
        }

        if first && !has_clef {
            // The clef at the start of a staff is generated when the file has
            // no explicit clef segment (`staff.defaultClefType()`).
            events.insert(
                0,
                Atom::simple(0, AtomKind::Clef { name: clef_name(clef) }),
            );
        }
        atoms.push(MeasureAtom {
            number: measure.number,
            width: x + SEGMENT_WIDTH,
            events,
            start_bar: bar_code(measure.start_bar_line),
            end_bar: bar_code(measure.end_bar_line),
        });
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
    }

    #[test]
    fn stem_length_is_at_least_shortest() {
        assert_eq!(stem_length(&[4], true, 4), 3072); // one stem shortening
        assert_eq!(stem_length(&[0], true, 4), 2560); // shortestStem
    }
}
