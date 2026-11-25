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

//! Derived modern-notation projection (L0/L1) of a GABC payload.
//!
//! GABC has **no metre, no measure numbering and no fixed durations**
//! (`Gabc.tex:801-845`): the only rhythmic facts are neume shapes, punctum
//! mora, episemata and bar weight (`:742-799`). The projection below is
//! therefore a deliberate choice external to the format: one `Note` per pitch
//! letter, a quarter-note default duration and one nominal 4/4 measure per
//! syllable. It is flagged both in the payload (`derived_modern_view`) and in
//! `Score::report`, so a downstream writer can tell it is not source data.

use scoretk_core::model::{
    BarLineType, Chord, ChordRest, ChordRestKind, Clef, ClefType, Duration, Fraction, Gregorian,
    Layers, Measure, Note, Part, Score, Segment, SegmentItem, SegmentType, Track,
};
use scoretk_gabc_common::syntax;

/// Builds the derived spine. The L6 payload is left untouched.
pub(super) fn build_projection(score: &mut Score, gregorian: &Gregorian) {
    let clef = gregorian.clefs.first().map_or("c4", String::as_str);
    let part = score.add_part(Part::new("Gregorian", "Gre"));
    let staff = score.add_staff(part);
    score.staves[staff.index()].lines = gregorian.staff_lines;
    let track = Track::new(staff, 0);

    let mut start = Fraction::ZERO;
    for (index, syllable) in gregorian.syllables.iter().enumerate() {
        let count = syllable.notes.len();
        let len = if count == 0 {
            Fraction::ZERO
        } else {
            Fraction::new(count as i64, 4)
        };
        let mut measure = Measure::new(index, start, Fraction::ONE).with_len(len);
        if let Some(bar) = syllable.bars.last() {
            measure.end_bar_line = bar_line(*bar);
        }
        for (note_index, note) in syllable.notes.iter().enumerate() {
            let step = syntax::diatonic_step(note.letter, clef);
            let id = score.add_note(Note::new(
                syntax::concert_pitch(step),
                syntax::tpc_of_step(step),
            ));
            let mut chord_rest = ChordRest::chord(Duration::quarter());
            chord_rest.kind = ChordRestKind::Chord(Chord {
                notes: vec![id],
                ..Chord::default()
            });
            let tick = Fraction::new(note_index as i64, 4);
            let mut segment = Segment::new(SegmentType::ChordRest, tick, 1);
            segment.set_element(track, SegmentItem::ChordRest(chord_rest));
            measure.add_segment(segment);
        }
        if index == 0 {
            let mut segment = Segment::new(SegmentType::Clef, Fraction::ZERO, 1);
            segment.set_element(track, SegmentItem::Clef(Clef::new(clef_type(clef))));
            measure.add_segment(segment);
        }
        start = measure.end_tick();
        score.add_measure(measure);
    }

    score.report.record(
        Layers::COMMON,
        "derived-modern-view: one note per pitch letter, quarter duration, nominal 4/4",
        None,
        None,
    );
}

/// Maps a GABC separation bar to the closest modern bar line.
///
/// The richer vocabulary (`;1`…`;8`, dotted divisio maior, divisio minimis)
/// is *not* represented by `BarLineType`: it stays in the L6 payload.
fn bar_line(bar: scoretk_core::model::BarKind) -> BarLineType {
    use scoretk_core::model::BarKind as B;
    match bar {
        B::DivisioFinalis => BarLineType::End,
        B::DivisioMaior | B::DivisioMaiorDot => BarLineType::Double,
        B::Dominican(_) => BarLineType::Dotted,
        _ => BarLineType::Normal,
    }
}

/// Closest modern clef to a GABC clef token (`Gabc.tex:847-870`).
fn clef_type(token: &str) -> ClefType {
    let first = token.split('@').next().unwrap_or(token);
    let line = first
        .bytes()
        .find(|byte| byte.is_ascii_digit())
        .unwrap_or(b'4')
        - b'0';
    if first.starts_with('f') {
        match line {
            3 => ClefType::FBaritone,
            5 => ClefType::FSubbass,
            _ => ClefType::F,
        }
    } else {
        match line {
            1 => ClefType::C1,
            2 => ClefType::C2,
            3 => ClefType::C3,
            5 => ClefType::C5,
            _ => ClefType::C4,
        }
    }
}
