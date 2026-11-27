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

use alloc::string::String;
use alloc::vec::Vec;

use scoretk_core::model::{
    AccidentalType, Annotation, BarLineType, BeamMode, ChordRestKind, ClefType, DurationType,
    NoteHeadType, Score, SegmentItem, SegmentType, StaffId, StemDirection, StyleSettings, Track,
    Transpose,
};

use scoretk_core::model::NoteHeadGroup;
use scoretk_smufl::{anchor, glyph, text_width, units_to_fixed};

use crate::style::{
    CLEF_KEY_RIGHT_MARGIN, CLEF_LEFT_MARGIN, KEYSIG_ACCIDENTAL_DISTANCE, KEY_TIME_DISTANCE,
    MIN_NOTE_DISTANCE, QUARTER_NOTE_SPACE, SEGMENT_WIDTH, SP, STEM_WIDTH, SYSTEM_HEADER_MIN_START,
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
    /// A cue/grace chord: smaller stem anchor (`graceNoteMag` 0.7).
    pub grace: bool,
    pub stem_up: bool,
    /// Stem length in 1/1024 staff space, already computed from the C++ rule.
    pub stem_len: i32,
    pub notes: Vec<NoteAtom>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomKind {
    Clef {
        name: &'static str,
    },
    KeySig {
        fifths: i8,
        /// Previous key, used to draw cancellation naturals
        /// (`measurelayout.cpp:2597-2660`, `keysig.cpp`).
        prev_fifths: i8,
    },
    TimeSig {
        numerator: u8,
        denominator: u8,
    },
    Chord,
    Rest {
        duration: u8,
        dots: u8,
        /// The source duration is `DurationType::Measure` (a full-measure
        /// rest), which MuseScore centres in the measure
        /// (`measurelayout.cpp:1501-1512`). A plain whole rest is not.
        measure: bool,
    },
}

/// One event of a measure atom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    /// x offset from the beginning of the measure, in fixed staff spaces.
    pub x: i32,
    pub kind: AtomKind,
    pub chord: Option<ChordAtom>,
    /// Voice (track) the event belongs to: even tracks stem up, odd tracks
    /// stem down (`chordlayout.cpp:1584-1588`).
    pub voice: u32,
    /// Source measure number, so beam groups do not silently cross a
    /// measure boundary (`beamlayout.cpp:660-700`).
    pub measure: usize,
    /// Number of beam strokes carried by this chord/rest: `beams()` in
    /// `engraving/dom/chordrest.cpp`, i.e. `duration - quarter`.
    pub beams: u8,
    /// `BeamMode` of the event, used to group beamed chords
    /// (`beamlayout.cpp:622-809`, `beamNoContinue`/`beamModeMid`).
    pub beam_mode: BeamMode,
    /// A `<Beam>` element attached to this event starts a new group.
    pub beam_start: bool,
    /// Explicit beam direction from `<Beam><StemDirection>`, if any.
    pub beam_dir: Option<StemDirection>,
    /// True for a cue/grace chord: smaller glyphs and stems
    /// (`beamlayout.cpp:135-340`).
    pub grace: bool,
    /// Index of the source segment in its measure: this is the shared
    /// horizontal alignment key between staves of a system, as `Segment*`
    /// positions are shared by all staves in MuseScore
    /// (`horizontalspacing.cpp:47-89`, `:262-327`).
    pub segment: u16,
    /// Lyrics syllables attached to this chord/rest (`LyricsLayout::layout`,
    /// `lyricslayout.cpp:45-170`); one entry per verse.
    pub lyrics: Vec<String>,
}

impl Default for Atom {
    fn default() -> Self {
        Self::simple(
            0,
            AtomKind::Rest {
                duration: 4,
                dots: 0,
                measure: false,
            },
        )
    }
}

impl Default for MeasureAtom {
    fn default() -> Self {
        Self {
            number: 0,
            width: 0,
            spring: 0,
            events: Vec::new(),
            start_bar: BarCode(0),
            end_bar: BarCode(0),
            line_break: false,
            clef_name: "gClef",
            key_fifths: 0,
            mm_rest_count: 1,
            segment_bases: Vec::new(),
            header_width: 0,
        }
    }
}

impl Atom {
    pub(crate) fn simple(x: i32, kind: AtomKind) -> Self {
        Self::at_segment(x, kind, 0)
    }

    pub(crate) fn at_segment(x: i32, kind: AtomKind, segment: u16) -> Self {
        Self {
            x,
            kind,
            chord: None,
            voice: 0,
            measure: 0,
            beams: 0,
            beam_mode: BeamMode::Auto,
            beam_start: false,
            beam_dir: None,
            grace: false,
            segment,
            lyrics: Vec::new(),
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
    /// Number of source measures folded into a multi-measure rest (1 when
    /// this is an ordinary measure). // Port of `Measure::isMMRest` and
    /// `MeasureLayout::layoutMeasure`'s centring step (`:1486-1491`).
    pub mm_rest_count: u16,
    /// Cursor x at the beginning of each source segment, indexed by segment
    /// number. Shared between staves by `align_staves` so a tick has one x
    /// for the whole system (`horizontalspacing.cpp:262-327`).
    pub segment_bases: Vec<i32>,
    /// Width of the clef/key header generated at a system start.
    pub header_width: i32,
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
                    prev_fifths: self.key_fifths,
                },
            ));
            header_width += self.key_fifths.unsigned_abs().min(7) as i32
                * (glyph_width(name) + KEYSIG_ACCIDENTAL_DISTANCE)
                + KEY_TIME_DISTANCE;
        }
        for event in &mut atom.events {
            event.x += header_width;
        }
        for base in &mut atom.segment_bases {
            *base += header_width;
        }
        atom.width += header_width;
        atom.header_width = header_width;
        // Header events keep their own absolute style x; they do not move
        // with the aligned chord segments (segment sentinel u16::MAX).
        for event in &mut events {
            event.segment = u16::MAX;
        }
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

/// // Port of `engraving/dom/note.cpp`'s `headGroup`/`headType` → SMuFL
/// `SymId` table (`noteheadXBlack`, `noteheadDiamondBlack`, …). The pivot
/// model keeps the group and the fill separately (`NoteHeadGroup`,
/// `NoteHeadType`); MuseScore's `Note::sym()` combines them.
fn head_name(head: NoteHeadType, group: &NoteHeadGroup, duration: DurationType) -> &'static str {
    let effective = if head == NoteHeadType::Auto {
        duration.head_type()
    } else {
        head
    };
    let filled = !matches!(
        effective,
        NoteHeadType::Breve | NoteHeadType::Whole | NoteHeadType::Half
    );
    let base = match group {
        NoteHeadGroup::Normal => match effective {
            NoteHeadType::Breve => "noteheadDoubleWhole",
            NoteHeadType::Whole => "noteheadWhole",
            NoteHeadType::Half => "noteheadHalf",
            _ => "noteheadBlack",
        },
        NoteHeadGroup::Cross => "noteheadXBlack",
        NoteHeadGroup::Plus => "noteheadPlusBlack",
        NoteHeadGroup::XCircle => "noteheadCircleX",
        NoteHeadGroup::WithX => "noteheadRoundWhiteDoubleSlashed",
        NoteHeadGroup::BrevisAlt => "noteheadDoubleWholeSquare",
        NoteHeadGroup::TriangleUp => {
            if filled {
                "noteheadTriangleUpBlack"
            } else {
                "noteheadTriangleUpWhite"
            }
        }
        NoteHeadGroup::TriangleDown => {
            if filled {
                "noteheadTriangleRoundDownBlack"
            } else {
                "noteheadTriangleRoundDownWhite"
            }
        }
        NoteHeadGroup::Slashed1 => {
            if filled {
                "noteheadSlashedBlack1"
            } else {
                "noteheadSlashedHalf1"
            }
        }
        NoteHeadGroup::Slashed2 => {
            if filled {
                "noteheadSlashedBlack2"
            } else {
                "noteheadSlashedHalf2"
            }
        }
        NoteHeadGroup::Diamond | NoteHeadGroup::DiamondOld => match effective {
            NoteHeadType::Breve => "noteheadDiamondDoubleWhole",
            NoteHeadType::Whole => "noteheadDiamondWhole",
            NoteHeadType::Half => "noteheadDiamondHalf",
            _ => "noteheadDiamondBlack",
        },
        NoteHeadGroup::Circled | NoteHeadGroup::CircledLarge => "noteheadCircledXLarge",
        NoteHeadGroup::LargeArrow | NoteHeadGroup::LargeDiamond => "noteheadTriangleRightBlack",
        NoteHeadGroup::HeavyCross | NoteHeadGroup::HeavyCrossHat => "noteheadClusterSquareBlack",
        NoteHeadGroup::Slash => "noteheadSlashedBlack1",
        NoteHeadGroup::Do
        | NoteHeadGroup::Re
        | NoteHeadGroup::Mi
        | NoteHeadGroup::Fa
        | NoteHeadGroup::Sol
        | NoteHeadGroup::La
        | NoteHeadGroup::Ti => {
            if filled {
                "noteheadTriangleUpBlack"
            } else {
                "noteheadTriangleUpWhite"
            }
        }
        NoteHeadGroup::Other(_) => "noteheadBlack",
    };
    if glyph(base).is_some() {
        base
    } else {
        "noteheadBlack"
    }
}

/// SMuFL stem anchor y of a head, in font units, up-positive.
/// // Port of `stemlayout.cpp:60-63, 220-228` (`extraHeight`, `stemPosX`):
/// MuseScore reads `stemUpSE`/`stemDownNW` from the head's metadata.
fn stem_anchor_units(name: &str, up: bool) -> i16 {
    let anchor_name = if up { "stemUpSE" } else { "stemDownNW" };
    glyph(name)
        .and_then(|g| anchor(g, anchor_name))
        .map(|a| a.1)
        .unwrap_or(40)
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

/// Default beam groups of a time signature, ported from `groups.cpp:43-100`
/// (`noteGroups`) and `:313-335` (`Groups::endings`). A node's `pos` is in
/// `Constants::DIVISION / 8` ticks (60 ticks per quarter at DIVISION 480),
/// and `action` holds four nibbles: one beam mode per duration from the
/// eighth to the thirty-second.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct BeamGroups {
    nodes: alloc::vec::Vec<(i32, u16)>,
}

impl BeamGroups {
    fn from_time_sig(time: &scoretk_core::model::TimeSig) -> Self {
        if let Some(explicit) = &time.groups {
            // MuseScore writes node positions in absolute ticks and divides
            // them by 60 in `Groups::addStop` (`groups.cpp:351-370`).
            let mut nodes: alloc::vec::Vec<_> = explicit
                .iter()
                .map(|&(pos, action)| (if pos >= 60 { pos / 60 } else { pos }, action))
                .collect();
            nodes.sort_by_key(|node| node.0);
            return Self { nodes };
        }
        let n = time.numerator;
        let d = time.denominator;
        let default = match (n, d) {
            (2, 2) => Some(
                &[
                    (4, 0x200),
                    (8, 0x110),
                    (12, 0x200),
                    (16, 0x111),
                    (20, 0x200),
                    (24, 0x110),
                    (28, 0x200),
                ][..],
            ),
            (3, 2) => Some(
                &[
                    (4, 0x200),
                    (8, 0x110),
                    (12, 0x200),
                    (16, 0x111),
                    (20, 0x200),
                    (24, 0x110),
                    (28, 0x200),
                    (32, 0x111),
                    (36, 0x200),
                    (40, 0x110),
                    (44, 0x200),
                ][..],
            ),
            (4, 2) => Some(
                &[
                    (4, 0x200),
                    (8, 0x110),
                    (12, 0x200),
                    (16, 0x111),
                    (20, 0x200),
                    (24, 0x110),
                    (28, 0x200),
                    (32, 0x111),
                    (36, 0x200),
                    (40, 0x110),
                    (44, 0x200),
                    (48, 0x111),
                    (52, 0x200),
                    (56, 0x110),
                    (60, 0x200),
                ][..],
            ),
            (2, 4) => Some(&[(4, 0x200), (8, 0x111), (12, 0x200)][..]),
            (3, 4) => Some(
                &[
                    (4, 0x200),
                    (8, 0x111),
                    (12, 0x200),
                    (16, 0x111),
                    (20, 0x200),
                ][..],
            ),
            (4, 4) => Some(
                &[
                    (4, 0x200),
                    (8, 0x110),
                    (12, 0x200),
                    (16, 0x111),
                    (20, 0x200),
                    (24, 0x110),
                    (28, 0x200),
                ][..],
            ),
            (5, 4) => Some(
                &[
                    (4, 0x200),
                    (8, 0x110),
                    (12, 0x200),
                    (16, 0x110),
                    (20, 0x200),
                    (24, 0x111),
                    (28, 0x200),
                    (32, 0x110),
                    (36, 0x200),
                ][..],
            ),
            (6, 4) => Some(
                &[
                    (4, 0x200),
                    (8, 0x110),
                    (12, 0x200),
                    (16, 0x110),
                    (20, 0x200),
                    (24, 0x111),
                    (28, 0x200),
                    (32, 0x110),
                    (36, 0x200),
                    (40, 0x110),
                    (44, 0x200),
                ][..],
            ),
            (3, 8) => Some(&[(4, 0x200), (8, 0x200)][..]),
            (5, 8) => Some(&[(4, 0x200), (8, 0x200), (12, 0x111), (16, 0x200)][..]),
            (6, 8) => Some(
                &[
                    (4, 0x200),
                    (8, 0x200),
                    (12, 0x111),
                    (16, 0x200),
                    (20, 0x200),
                ][..],
            ),
            (7, 8) => Some(
                &[
                    (4, 0x200),
                    (8, 0x200),
                    (12, 0x111),
                    (16, 0x200),
                    (20, 0x111),
                    (24, 0x200),
                ][..],
            ),
            (9, 8) => Some(
                &[
                    (4, 0x200),
                    (8, 0x200),
                    (12, 0x111),
                    (16, 0x200),
                    (20, 0x200),
                    (24, 0x111),
                    (28, 0x200),
                    (32, 0x200),
                ][..],
            ),
            (12, 8) => Some(
                &[
                    (4, 0x200),
                    (8, 0x200),
                    (12, 0x111),
                    (16, 0x200),
                    (20, 0x200),
                    (24, 0x111),
                    (28, 0x200),
                    (32, 0x200),
                    (36, 0x111),
                    (40, 0x200),
                    (44, 0x200),
                ][..],
            ),
            _ => None,
        };
        if let Some(nodes) = default {
            return Self {
                nodes: nodes.to_vec(),
            };
        }
        // `Groups::endings`: one stop at each beat of an unlisted signature.
        let step = match d {
            2 => 16,
            4 => 8,
            8 => 4,
            16 => 2,
            32 => 1,
            _ => 4,
        };
        let nodes = (1..n.max(1)).map(|i| (step * i, 0x111)).collect();
        Self { nodes }
    }

    /// `Groups::beamMode` (`groups.cpp:266-307`): the action nibble selected
    /// by the chord's duration (eighth, sixteenth, thirty-second or shorter).
    fn beam_mode(&self, tick: i32, duration: u8) -> BeamMode {
        let shift = match duration {
            5 => 0,
            6 => 4,
            _ => 8,
        };
        for &(pos, action) in &self.nodes {
            if pos * 60 < tick {
                continue;
            }
            if pos * 60 > tick {
                break;
            }
            return match (action >> shift) & 0xf {
                1 => BeamMode::Begin,
                2 => BeamMode::Begin16,
                3 => BeamMode::Begin32,
                _ => BeamMode::Auto,
            };
        }
        BeamMode::Auto
    }
}

/// Actual duration in MuseScore ticks (480 per quarter, 1920 per whole),
/// used by `Groups::baseBeamMode` to find the beat subdivision.
fn duration_ticks(duration: u8, dots: u8) -> i32 {
    if duration < 2 {
        return 1920;
    }
    let base = 1920i32 >> (duration - 2).min(10);
    let factor = match dots {
        1 => 3,
        2 => 7,
        _ => 1,
    };
    base * factor / (1 << dots.min(2))
}

/// Port of `Groups::baseBeamMode` (`groups.cpp:107-162`) and
/// `Groups::actualBeamMode` (`:199-263`): an explicit mode always wins;
/// automatic chords follow the time-signature groups, with a beam starting
/// at tick zero and continuing as `Mid` elsewhere. Rests with an automatic
/// mode are not beamed (`actualBeamMode`, `:215-218`).
fn beam_mode_for(
    explicit: BeamMode,
    is_rest: bool,
    duration: u8,
    dots: u8,
    tick: i32,
    prev_ticks: i32,
    groups: &BeamGroups,
) -> BeamMode {
    if !matches!(explicit, BeamMode::Auto) {
        return explicit;
    }
    if is_rest || duration <= 4 {
        return BeamMode::None;
    }
    let mut beat = 240; // start with an eighth (`groups.cpp:112`)
    while beat > 0 && (beat > duration_ticks(duration, dots).max(prev_ticks) || tick % beat != 0) {
        beat /= 2;
        if beat < 60 {
            beat = duration_ticks(duration, dots).max(1);
            break;
        }
    }
    let beat_duration = match beat {
        1920 => 2,
        960 => 3,
        480 => 4,
        240 => 5,
        120 => 6,
        _ => 7,
    };
    let by_type = groups.beam_mode(tick, duration);
    let by_pos = groups.beam_mode(tick, beat_duration);
    let mut mode = if matches!(by_type, BeamMode::Auto) {
        by_pos
    } else {
        by_type
    };
    if matches!(mode, BeamMode::Auto) {
        mode = if tick == 0 {
            BeamMode::Begin
        } else {
            BeamMode::Mid
        };
    }
    mode
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
    let extra = (count.saturating_sub(2) as i32).clamp(0, 8) * quarters.max(1);
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
pub(crate) fn auto_stem_up(lines: &[i32]) -> bool {
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
    // `computeAutoStemDirection` returns 0 when the notes cancel out, and
    // `computeUp` sets `up = direction > 0`: a note on the middle line (or a
    // symmetric chord) gets a **down** stem. The old default of `true` put
    // every middle-line stem on the wrong side of its notehead.
    false
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

/// One chord of a segment, reduced to the fields used by the up/down voice
/// collision rules of `ChordLayout::calculateChordOffsets`.
#[derive(Clone, Copy, Debug)]
struct CollisionChord {
    up: bool,
    /// Lowest line of an up-stem chord (largest line number).
    low: i32,
    /// Highest line of a down-stem chord (smallest line number).
    high: i32,
    dots: u8,
    duration: u8,
    notes: usize,
}

/// Horizontal offsets of chords that collide with the opposite-stem voice,
/// ported from `ChordLayout::calculateChordOffsets`
/// (`chordlayout.cpp:1887-2130`). Unisons with the same head, duration and
/// dots share one notehead (`shareHeads`); a second shifts the down-stem
/// chord so that the two stems align; overlapping notes shift the up-stem
/// chord by a head width plus 0.15 sp.
fn voice_collision_offsets(chords: &[CollisionChord]) -> alloc::vec::Vec<i32> {
    let mut offsets = alloc::vec![0; chords.len()];
    let head = glyph_width("noteheadBlack");
    let ups: alloc::vec::Vec<usize> = chords
        .iter()
        .enumerate()
        .filter(|(_, chord)| chord.up)
        .map(|(index, _)| index)
        .collect();
    let downs: alloc::vec::Vec<usize> = chords
        .iter()
        .enumerate()
        .filter(|(_, chord)| !chord.up)
        .map(|(index, _)| index)
        .collect();
    if ups.is_empty() || downs.is_empty() {
        return offsets;
    }
    let bottom_up = ups
        .iter()
        .map(|&index| chords[index].low)
        .max()
        .unwrap_or(0);
    let top_down = downs
        .iter()
        .map(|&index| chords[index].high)
        .min()
        .unwrap_or(0);
    let separation = top_down - bottom_up;
    let up_dots = ups.iter().any(|&index| chords[index].dots > 0);
    let down_dots = downs.iter().any(|&index| chords[index].dots > 0);
    if separation == 1 {
        if up_dots && !down_dots {
            for &index in &ups {
                offsets[index] = head + SP / 10;
            }
        } else {
            // Align the stems of the two voices (`chordlayout.cpp:1902-1912`).
            for &index in &downs {
                offsets[index] = head - STEM_WIDTH;
            }
        }
    } else if separation <= 0 {
        let shareable = ups.iter().all(|&up| {
            downs.iter().all(|&down| {
                let a = chords[up];
                let b = chords[down];
                a.duration == b.duration && a.dots == b.dots && a.notes == 1 && b.notes == 1
            })
        });
        if !shareable {
            for &index in &ups {
                offsets[index] = head + 154; // 0.15 sp (`chordlayout.cpp:2090`)
            }
        }
    }
    offsets
}

/// // Port of `stemlayout.cpp:42-160` (`StemLayout::calcDefaultStemLength`):
/// all internal arithmetic is in quarter spaces; with line distance 1 and
/// magnitude 1, `defaultStemLength = stemLength * 4 = 14`, `quarterSpacesPerLine
/// = floor(lineDistance * 2) = 2`, `chordHeight = (downLine - upLine) * 2`,
/// `shortestStem = 2.5 * 4 = 10` and the final length is
/// `chordHeight / 4 * sp + stemLength / 4 * sp`, to which the middle-line
/// extension of `:122-159` is added. Beams and tremolos are P3.
/// Hooks and stem shortening follow `stemlayout.cpp:326-397` below.
fn stem_length(lines: &[i32], heads: &[&str], up: bool, duration: u8) -> i32 {
    if lines.is_empty() {
        return 3584;
    }
    // // Port of `stemlayout.cpp:60-63, 345-373`: the head's SMuFL stem
    // anchor adds to `shortestStem` (2.5 sp * 4 quarter spaces).
    let anchor_head = if up { heads.first() } else { heads.last() };
    let anchor_units = anchor_head.map_or(40, |name| stem_anchor_units(name, up));
    let min_qs = 10 + (i32::from(anchor_units).abs() * 4 / 250).max(0);
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
            reduction = reduction.min(2);
        }
        stem_qs = (stem_qs - reduction).max(min_qs);
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

/// Note atoms of one chord, with the written pitch (transposed back to the
/// written spelling) and the SMuFL head for its duration.
fn note_atoms(
    chord: &scoretk_core::model::Chord,
    score: &Score,
    duration: DurationType,
    bottom: i32,
    inverse: Option<Transpose>,
) -> Vec<NoteAtom> {
    let mut notes = Vec::new();
    for id in &chord.notes {
        let Some(note) = score.notes.get(id.index()) else {
            continue;
        };
        if !note.visible {
            continue;
        }
        let head = head_name(note.head, &note.head_group, duration);
        let accidental = note
            .accidental
            .filter(|a| a.visible)
            .and_then(|a| accidental_name(a.kind));
        let (written_tpc, written_pitch) = match inverse {
            Some(interval) => {
                let _ = interval.diatonic;
                (
                    note.tpc,
                    (i32::from(note.pitch) + interval.chromatic).clamp(0, 127) as u8,
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
    notes.sort_by_key(|note| note.line);
    notes
}

/// Number of naturals MuseScore draws to cancel the previous key before a
/// new one (`KeySig::layout`, `measurelayout.cpp:2597-2660`).
pub(crate) fn cancellation_count(previous: i8, next: i8) -> u8 {
    let removed_sharps = if previous > 0 && next < previous {
        (previous - next.max(0)).unsigned_abs()
    } else if previous < 0 && next > 0 {
        previous.unsigned_abs()
    } else {
        0
    };
    let removed_flats = if previous < 0 && next > previous {
        (next.min(0) - previous).unsigned_abs()
    } else if previous > 0 && next < 0 {
        previous.unsigned_abs()
    } else {
        0
    };
    removed_sharps.min(7) + removed_flats.min(7)
}

/// Whether measured text shapes widen a measure's break width. Disabled
/// after the P4 measurement campaign (§9.23 of `docs/agents/engraving.md`):
/// with the faithful Edwin advances the best variant kept 34/44 system breaks
/// but reduced within-0.1-sp positions from 365 to 350. The widths and the
/// helper are ready for the full `computePadding`/`Skyline` port in P4.
pub const TEXT_WIDTH_SPACING: bool = false;

/// Font em in fixed staff spaces for a point size (`styledef.cpp:1209-1223`).
fn text_em(style: &StyleSettings, font_size: f64) -> i32 {
    let spatium = style.spatium_mm.max(0.1);
    ((font_size * 25.4 / 72.0) / spatium * SP as f64) as i32
}

/// Widths of the three text families attached to a chord/rest segment:
/// lyrics, harmony symbols and tempo. Index 0/1/2 respectively; zero means
/// the segment carries none of that family.
///
/// // Port of `horizontalspacing.cpp:1438-1604` (`computePadding`): the real
/// Edwin advances give each item's shape width. A character-count estimate
/// previously moved 17 of 44 system breaks away from the oracle; even adding
/// the measured full width to the segment still moved 3 of 44, because
/// MuseScore only separates *neighbouring* shapes that would overlap.
fn attached_text_widths(segment: &scoretk_core::model::Segment, style: &StyleSettings) -> [i32; 3] {
    let lyric_em = text_em(style, style.lyricist_font_size.max(10.0));
    let mut widths = [0; 3];
    if segment.kind == SegmentType::ChordRest {
        for (_, item) in segment.iter_elements() {
            let SegmentItem::ChordRest(chord_rest) = item else {
                continue;
            };
            for lyrics in &chord_rest.lyrics {
                widths[0] = widths[0].max(text_width(&lyrics.text, lyric_em, false));
            }
        }
    }
    let harmony_em = text_em(style, 12.0);
    let tempo_em = text_em(style, 12.0);
    for annotation in &segment.annotations {
        let index = match &annotation.annotation {
            Annotation::Harmony(_) => Some(1),
            Annotation::Tempo(_) => Some(2),
            _ => None,
        };
        if let Some(index) = index {
            let em = if index == 1 { harmony_em } else { tempo_em };
            widths[index] = widths[index].max(text_width(annotation.text(), em, false));
        }
    }
    widths
}

/// Indices of the staves MuseScore actually engraves: a hidden/invisible
/// staff is skipped both in `SystemLayout::layoutSystem`
/// (`systemlayout.cpp:2477-2485`) and in `PageLayout::distributeStaves`
/// (`pagelayout.cpp:661`). Parts and excerpts are already flattened into
/// `score.staves`; hidden staves are *not* removed by the readers.
pub fn visible_staves(score: &Score) -> Vec<usize> {
    (0..score.staves.len())
        .filter(|&index| score.staff_shown(StaffId::from_raw(index as u32)))
        .collect()
}

/// Computes the measure atoms of the given staff.
fn compute_staff(score: &Score, staff_index: usize) -> Vec<MeasureAtom> {
    let staff = &score.staves[staff_index];
    let mut clef = staff.default_clef.concert;
    if clef == ClefType::Invalid {
        clef = ClefType::G;
    }
    let mut bottom = bottom_line_note(clef);
    let mut key_fifths = 0i8;
    let mut prev_fifths = 0i8;
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
    // Beam groups in force for this staff. MuseScore's `Staff::group(tick)`
    // selects them per time-signature segment (`groups.cpp:107-162`); a
    // page-local atom keeps the effective beam mode of each event instead.
    let mut groups = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .find_map(|segment| {
            segment.iter_elements().find_map(|(_, item)| match item {
                SegmentItem::TimeSig(time) => Some(BeamGroups::from_time_sig(time)),
                _ => None,
            })
        })
        .unwrap_or_default();
    let mut atoms = Vec::with_capacity(score.measures.len());

    let mut measure_index = 0;
    while let Some(measure) = score.measures.get(measure_index) {
        // A `<multiMeasureRest>N</multiMeasureRest>` folds N source measures
        // into one displayed measure (`measurelayout.cpp:194-524`).
        let mm_count = measure.multi_measure_rest.unwrap_or(1).max(1) as usize;
        // A line break carried by a folded measure still ends the displayed
        // system after the multi-measure rest.
        let mut line_break = measure.line_break;
        for skipped in
            &score.measures[measure_index + 1..(measure_index + mm_count).min(score.measures.len())]
        {
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
            events.push(Atom::at_segment(clef_x, AtomKind::Clef { name }, 0));
            x = clef_x + glyph_width(name) + CLEF_KEY_RIGHT_MARGIN + SP / 10;
            has_clef = true;
        }
        // // Port of `Measure::hasVoices(staffIdx)` (`chordlayout.cpp:1584`):
        // a measure has voices as soon as two distinct tracks carry a
        // chord/rest, even when they never share a segment. `linked-ties`
        // writes voice 2 in its own segments, and MuseScore still forces
        // their stems by track parity.
        let mut used_tracks = alloc::vec::Vec::new();
        for segment in &measure.segments {
            for (track, item) in segment.iter_elements() {
                if track.staff().index() != staff_index {
                    continue;
                }
                if matches!(item, SegmentItem::ChordRest(_)) && !used_tracks.contains(&track.raw())
                {
                    used_tracks.push(track.raw());
                }
            }
        }
        let multi_voice = used_tracks.len() > 1;
        // (x of the last segment carrying this text family, shape width).
        let mut last_text: [(i32, i32); 3] = [(0, 0); 3];
        let mut text_extra = 0;
        let mut segment_bases = Vec::with_capacity(measure.segments.len());
        // Previous chord/rest of each voice in this measure: `baseBeamMode`
        // compares its duration with the current one (`groups.cpp:107-125`).
        let mut prev_beam: alloc::vec::Vec<(u32, i32)> = alloc::vec::Vec::new();

        for (segment_number, segment) in measure.segments.iter().enumerate() {
            segment_bases.push(x);
            let seg = segment_number as u16;
            let tick = (segment.tick - measure.start_tick).ticks(1920) as i32;
            let segment_staff = |track: Track| track.staff().index() == staff_index;
            if segment.kind == SegmentType::ChordRest {
                // P1: fixed proportional spacing, one x per chord/rest segment
                // (plan §4, stage 1; P2 replaces this with real springs). All
                // voices of a segment share that x; MuseScore forces their
                // stem directions by track parity (`chordlayout.cpp:1584`).
                let chord_rests: Vec<(u32, &_)> = segment
                    .iter_elements()
                    .filter(|(track, _)| segment_staff(*track))
                    .filter_map(|(track, item)| match item {
                        SegmentItem::ChordRest(cr) => Some((track.voice() as u32, cr)),
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
                let text_widths = if TEXT_WIDTH_SPACING {
                    attached_text_widths(segment, style)
                } else {
                    [0; 3]
                };
                let short_notes = segment.iter_elements().any(|(_, item)| {
                    matches!(item, SegmentItem::ChordRest(chord_rest)
                        if duration_code(chord_rest.duration.kind) >= 5)
                });
                // `minDistance` between neighbouring text shapes of the same
                // family: only an actual overlap widens the segment, as in
                // `HorizontalSpacing::computePadding` (not the full width).
                for (index, &width) in text_widths.iter().enumerate() {
                    if width == 0 || last_text[index].1 == 0 || (index == 0 && !short_notes) {
                        continue;
                    }
                    let needed = (last_text[index].1 + width) / 2;
                    let have = x - last_text[index].0;
                    if have < needed {
                        // MuseScore records this as extra free space in the
                        // measure (affecting its break width), but keeps the
                        // chord segments themselves at their spring positions.
                        text_extra += needed - have;
                    }
                }
                for (index, &width) in text_widths.iter().enumerate() {
                    if width > 0 && (index != 0 || short_notes) {
                        last_text[index] = (x, width);
                    }
                }
                let segment_start = events.len();
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
                            // // Port of `ChordLayout::layoutChords1`
                            // (`chordlayout.cpp:631-645`): grace notes before
                            // the main note are stacked to its left, one
                            // `minNoteDistance` apart, at `graceNoteMag` 0.7.
                            // They share the main note's segment and do not
                            // advance its x.
                            let head_w = glyph_width("noteheadBlack");
                            let grace_w = head_w * 7 / 10;
                            let mut grace_x = x - head_w - MIN_NOTE_DISTANCE - grace_w;
                            for grace in chord.grace_chords.iter().rev() {
                                let notes = note_atoms(
                                    &grace.chord,
                                    score,
                                    grace.duration.kind,
                                    bottom,
                                    inverse,
                                );
                                if !notes.is_empty() {
                                    let lines: Vec<i32> =
                                        notes.iter().map(|note| note.line).collect();
                                    let heads: Vec<&str> =
                                        notes.iter().map(|note| note.head).collect();
                                    let g_duration = duration_code(grace.duration.kind);
                                    let g_up = stem_up(&lines, StemDirection::Auto, false, track);
                                    // Measured against the 4.7.4 SVG: grace
                                    // stems are ~0.82 of a full stem after the
                                    // anchor and minimum-length corrections
                                    // (`stemlayout.cpp:60-161`, intrinsicMag
                                    // 0.7 with a different middle-line rule).
                                    let stem_len =
                                        stem_length(&lines, &heads, g_up, g_duration) * 82 / 100;
                                    events.push(Atom {
                                        x: grace_x,
                                        kind: AtomKind::Chord,
                                        chord: Some(ChordAtom {
                                            duration: g_duration,
                                            dots: grace.duration.dots,
                                            grace: true,
                                            stem_up: g_up,
                                            stem_len,
                                            notes,
                                        }),
                                        voice: track,
                                        measure: measure.number,
                                        beams: g_duration.saturating_sub(4),
                                        beam_mode: BeamMode::Auto,
                                        beam_start: false,
                                        beam_dir: None,
                                        grace: true,
                                        segment: seg,
                                        lyrics: Vec::new(),
                                    });
                                }
                                grace_x -= grace_w + MIN_NOTE_DISTANCE * 7 / 10;
                            }
                            let notes = note_atoms(chord, score, cr.duration.kind, bottom, inverse);
                            let lines: Vec<i32> = notes.iter().map(|n| n.line).collect();
                            let chord_x = x;
                            let heads: Vec<&str> = notes.iter().map(|n| n.head).collect();
                            let up = stem_up(&lines, chord.engraving.stem, multi_voice, track);
                            let chord_atom = ChordAtom {
                                duration,
                                dots: cr.duration.dots,
                                grace: false,
                                stem_up: up,
                                stem_len: stem_length(&lines, &heads, up, duration),
                                notes,
                            };
                            let mut chord_event = Atom {
                                x: chord_x,
                                kind: AtomKind::Chord,
                                chord: Some(chord_atom),
                                voice: track,
                                measure: measure.number,
                                // // Port of `ChordRest::beams()` in
                                // `engraving/dom/chordrest.cpp`: one stroke
                                // per duration step below a quarter.
                                beams: duration.saturating_sub(4),
                                beam_mode: cr.engraving.beam_mode,
                                beam_start: cr.engraving.beam_start,
                                beam_dir: cr.engraving.beam_direction,
                                grace: chord.grace.is_some(),
                                segment: seg,
                                lyrics: cr.lyrics.iter().map(|lyric| lyric.text.clone()).collect(),
                            };
                            let voice = track;
                            let prev = prev_beam
                                .iter()
                                .rev()
                                .find(|(track, _)| *track == voice)
                                .map_or(0, |&(_, ticks)| ticks);
                            if !chord_event.grace {
                                chord_event.beam_mode = beam_mode_for(
                                    chord_event.beam_mode,
                                    false,
                                    duration,
                                    cr.duration.dots,
                                    tick,
                                    prev,
                                    &groups,
                                );
                                prev_beam.push((voice, duration_ticks(duration, cr.duration.dots)));
                            }
                            events.push(chord_event);
                        }
                        ChordRestKind::Rest(_) => {
                            let mut rest = Atom::at_segment(
                                x,
                                AtomKind::Rest {
                                    duration,
                                    dots: cr.duration.dots,
                                    measure: cr.duration.kind == DurationType::Measure,
                                },
                                seg,
                            );
                            rest.voice = track;
                            rest.measure = measure.number;
                            rest.beams = duration.saturating_sub(4);
                            let prev = prev_beam
                                .iter()
                                .rev()
                                .find(|(voice, _)| *voice == track)
                                .map_or(0, |&(_, ticks)| ticks);
                            rest.beam_mode = beam_mode_for(
                                cr.engraving.beam_mode,
                                true,
                                duration,
                                cr.duration.dots,
                                tick,
                                prev,
                                &groups,
                            );
                            prev_beam.push((track, duration_ticks(duration, cr.duration.dots)));
                            rest.beam_start = cr.engraving.beam_start;
                            rest.beam_dir = cr.engraving.beam_direction;
                            rest.lyrics =
                                cr.lyrics.iter().map(|lyric| lyric.text.clone()).collect();
                            events.push(rest);
                        }
                    }
                }
                // // Port of `ChordLayout::layoutChords1`
                // (`chordlayout.cpp:2228-2290`): resolve unison and second
                // collisions between the up- and down-stem voices of this
                // segment. All voices keep their shared segment x, and only
                // the colliding chord's notehead/stem is shifted.
                let mut colliding: alloc::vec::Vec<(usize, CollisionChord)> =
                    alloc::vec::Vec::new();
                for (index, event) in events.iter().enumerate().skip(segment_start) {
                    if let Some(chord) = &event.chord {
                        if chord.grace {
                            continue;
                        }
                        let low = chord.notes.iter().map(|note| note.line).max().unwrap_or(4);
                        let high = chord.notes.iter().map(|note| note.line).min().unwrap_or(4);
                        colliding.push((
                            index,
                            CollisionChord {
                                up: chord.stem_up,
                                low,
                                high,
                                dots: chord.dots,
                                duration: chord.duration,
                                notes: chord.notes.len(),
                            },
                        ));
                    }
                }
                let info: alloc::vec::Vec<_> = colliding.iter().map(|&(_, chord)| chord).collect();
                for ((index, _), offset) in colliding.iter().zip(voice_collision_offsets(&info)) {
                    if offset != 0 {
                        events[*index].x += offset;
                    }
                }
                x += segment_width;
                continue;
            }

            for (_track, item) in segment
                .iter_elements()
                .filter(|(track, _)| segment_staff(*track))
            {
                match item {
                    SegmentItem::Clef(new_clef) if segment.kind.is_clef_type() && !has_clef => {
                        clef = new_clef.concert;
                        bottom = bottom_line_note(clef);
                        let name = clef_name(clef);
                        let clef_x = x + CLEF_LEFT_MARGIN;
                        events.push(Atom::at_segment(clef_x, AtomKind::Clef { name }, seg));
                        x = clef_x + glyph_width(name) + CLEF_KEY_RIGHT_MARGIN + SP / 10;
                        has_clef = true;
                    }
                    SegmentItem::KeySig(key) if segment.kind == SegmentType::KeySig && !has_key => {
                        let fifths = key.concert_key.fifths();
                        let previous = prev_fifths;
                        key_fifths = fifths;
                        prev_fifths = fifths;
                        if fifths != 0 || previous != 0 {
                            events.push(Atom::at_segment(
                                x,
                                AtomKind::KeySig {
                                    fifths,
                                    prev_fifths: previous,
                                },
                                seg,
                            ));
                            // // Port of `styledef.cpp:241` (`keysigAccidentalDistance`):
                            // consecutive accidentals are one glyph width plus
                            // 0.3 sp apart; `keyTimesigDistance` follows them.
                            let accidental = if fifths > 0 {
                                "accidentalSharp"
                            } else {
                                "accidentalFlat"
                            };
                            let count = fifths.unsigned_abs().min(7) as i32;
                            let cancelled = cancellation_count(previous, fifths);
                            x += (count + cancelled as i32)
                                * (glyph_width(accidental) + KEYSIG_ACCIDENTAL_DISTANCE)
                                + KEY_TIME_DISTANCE;
                        }
                        has_key = true;
                    }
                    SegmentItem::TimeSig(time)
                        if segment.kind == SegmentType::TimeSig && !has_time =>
                    {
                        groups = BeamGroups::from_time_sig(time);
                        let numerator = u8::try_from(time.numerator).unwrap_or(4);
                        let denominator = u8::try_from(time.denominator).unwrap_or(4);
                        events.push(Atom::at_segment(
                            x,
                            AtomKind::TimeSig {
                                numerator,
                                denominator,
                            },
                            seg,
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
            width: x + text_extra,
            spring,
            events,
            start_bar: bar_code(measure.start_bar_line),
            end_bar: bar_code(measure.end_bar_line),
            line_break,
            clef_name: clef_name(clef),
            key_fifths,
            mm_rest_count: mm_count.min(u16::MAX as usize) as u16,
            segment_bases,
            header_width: 0,
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

/// Computes the atoms of every engraved staff and aligns their segment
/// positions. // Port of `HorizontalSpacing::computeMinWidth` and
/// `collectSegments` (`horizontalspacing.cpp:47-89, 262-327`): MuseScore
/// visits the segments of a system once, taking the largest minimum width
/// over all staves at each tick, then assigns that x to every staff. The
/// pivot model shares its segments between staves, so the segment index is
/// the alignment key.
pub fn compute(score: &Score) -> Vec<Vec<MeasureAtom>> {
    let mut all: Vec<Vec<MeasureAtom>> = visible_staves(score)
        .into_iter()
        .map(|index| compute_staff(score, index))
        .collect();
    if all.len() < 2 {
        return all;
    }
    for measure in 0..all[0].len() {
        let tails: Vec<i32> = all
            .iter()
            .map(|atoms| {
                let atom = &atoms[measure];
                atom.width - atom.segment_bases.last().copied().unwrap_or(atom.width)
            })
            .collect();
        let count = all
            .iter()
            .map(|atoms| atoms[measure].segment_bases.len())
            .max()
            .unwrap_or(0);
        for segment in 0..count {
            let common = all
                .iter()
                .filter_map(|atoms| atoms[measure].segment_bases.get(segment))
                .copied()
                .max()
                .unwrap_or(0);
            for atoms in &mut all {
                let Some(base) = atoms[measure].segment_bases.get_mut(segment) else {
                    continue;
                };
                let delta = common - *base;
                *base = common;
                if delta != 0 {
                    for event in &mut atoms[measure].events {
                        if usize::from(event.segment) == segment {
                            event.x += delta;
                        }
                    }
                }
            }
        }
        // The measure ends where the last (possibly shifted) segment ends.
        for (staff, atoms) in all.iter_mut().enumerate() {
            let atom = &mut atoms[measure];
            let last_base = atom.segment_bases.last().copied().unwrap_or(0);
            atom.width = last_base + tails[staff].max(0);
        }
    }
    all
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
        // A symmetric chord (or a single note on the middle line) has no
        // net direction: `computeAutoStemDirection` returns 0 and the stem
        // points down (`chordlayout.cpp:1626-1642`).
        assert!(!auto_stem_up(&[2, 6]));
        assert!(!auto_stem_up(&[4]));
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
        assert_eq!(stem_length(&[4], &["noteheadBlack"], true, 4), 3072); // one stem shortening
        assert_eq!(stem_length(&[0], &["noteheadBlack"], true, 4), 2560); // shortestStem
    }

    #[test]
    fn stem_reaches_the_middle_line_from_outside_the_staff() {
        // `stemlayout.cpp:122-159`: a stem starting 2.5 sp above the top
        // line is extended by 3.5 sp so that its far end reaches the middle
        // line. Without the extension it would stop 1.5 sp short of it.
        assert_eq!(stem_length(&[-10], &["noteheadBlack"], false, 4), 28 * 256);
        // A stem starting 2 sp below the bottom line gains 0.5 sp.
        assert_eq!(stem_length(&[12], &["noteheadBlack"], true, 4), 16 * 256);
        // A note inside the staff only gets the shortening rule: 3 sp.
        assert_eq!(stem_length(&[4], &["noteheadBlack"], false, 4), 12 * 256);
    }

    #[test]
    fn voice_collisions_follow_the_up_down_chord_rules() {
        let head = glyph_width("noteheadBlack");
        let chord = |_voice, up, line, dots| CollisionChord {
            up,
            low: line,
            high: line,
            dots,
            duration: 4,
            notes: 1,
        };
        // A shared unison keeps one notehead x.
        let unison = [chord(0, true, 4, 0), chord(1, false, 4, 0)];
        assert_eq!(voice_collision_offsets(&unison), alloc::vec![0, 0]);
        // A second: the down-stem chord shifts so that both stems align.
        let second = [chord(0, true, 4, 0), chord(1, false, 5, 0)];
        assert_eq!(
            voice_collision_offsets(&second),
            alloc::vec![0, head - STEM_WIDTH]
        );
        // An overlap that cannot share heads shifts the up-stem chord.
        let overlap = [chord(0, true, 4, 0), chord(1, false, 4, 1)];
        assert_eq!(
            voice_collision_offsets(&overlap),
            alloc::vec![head + 154, 0]
        );
    }

    #[test]
    fn default_beam_groups_split_compound_metres_into_beats() {
        // `groups.cpp:43-100`: 12/8 and 6/8 start new groups after three
        // eighths, 9/8 after three, 3/2 after half-note beats.
        let compound = BeamGroups::from_time_sig(&scoretk_core::model::TimeSig::new(12, 8));
        assert!(compound.nodes.contains(&(12, 0x111)));
        assert!(compound.nodes.contains(&(24, 0x111)));
        assert_eq!(
            beam_mode_for(BeamMode::Auto, false, 5, 0, 0, 0, &compound),
            BeamMode::Begin
        );
        assert_eq!(
            beam_mode_for(BeamMode::Auto, false, 5, 0, 240, 240, &compound),
            BeamMode::Mid
        );
        assert_eq!(
            beam_mode_for(BeamMode::Auto, false, 5, 0, 720, 240, &compound),
            BeamMode::Begin
        );
        for (numerator, denominator, stop) in [(6, 8, 12), (9, 8, 24), (3, 2, 8)] {
            let groups = BeamGroups::from_time_sig(&scoretk_core::model::TimeSig::new(
                numerator,
                denominator,
            ));
            assert!(groups.nodes.iter().any(|&(pos, action)| pos == stop
                && action == if denominator == 2 { 0x110 } else { 0x111 }));
        }
        // An explicit `<Groups>` node overrides the default table.
        let mut explicit = scoretk_core::model::TimeSig::new(12, 8);
        explicit.groups = Some(alloc::vec![(240, 0x111)]);
        let groups = BeamGroups::from_time_sig(&explicit);
        assert_eq!(groups.nodes, alloc::vec![(4, 0x111)]);
        // Rests with automatic mode are not beamed; an explicit mode still is.
        assert_eq!(
            beam_mode_for(BeamMode::Auto, true, 5, 0, 240, 240, &compound),
            BeamMode::None
        );
        assert_eq!(
            beam_mode_for(BeamMode::Mid, true, 5, 0, 240, 240, &compound),
            BeamMode::Mid
        );
    }
}
