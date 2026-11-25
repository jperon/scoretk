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
    Layers, Lyrics, LyricsSyllabic, Measure, Note, Part, Score, Segment, SegmentItem, SegmentType,
    Track,
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
        // Even a bar-only group occupies one quarter: a zero-length measure
        // would put the next syllable at the same absolute tick.
        let len = Fraction::new(count.max(1) as i64, 4);
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
            // Segment ticks are absolute in the pivot (`pass2.rs:495-518`),
            // not relative to the measure: a relative tick would stack every
            // syllable's notes at the same MIDI onset.
            let tick = start + Fraction::new(note_index as i64, 4);
            if note_index == 0 {
                // GABC's syllable text is the lyric of the first note of its
                // neume (`Gabc.tex:228-245`). Keeping it in L3 lets a plain
                // MIDI/MusicXML writer emit words instead of dropping them;
                // the markup itself stays in the L6 payload.
                let text = plain_lyric(&syllable.text);
                if !text.is_empty() {
                    chord_rest
                        .lyrics
                        .push(Lyrics::new(0, LyricsSyllabic::Single, text));
                }
            }
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

/// Strips GABC typography from a syllable text to get the sung word
/// (`Gabc.tex:247-309,314-343`): `<sc>…</sc>` keeps its content, `$` escapes
/// and centring braces are removed, tags themselves are not lyrics.
fn plain_lyric(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut stack: Vec<String> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '<' => {
                if let Some(end) = chars[index..].iter().position(|char| *char == '>') {
                    let end = index + end;
                    let tag: String = chars[index + 1..end].iter().collect();
                    let name = tag
                        .trim_start_matches('/')
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if tag.starts_with('/') {
                        stack.pop();
                    } else {
                        stack.push(name);
                    }
                    index = end + 1;
                    continue;
                }
                index += 1;
            }
            '$' => {
                index += 1;
                if index < chars.len() {
                    out.push(chars[index]);
                    index += 1;
                }
            }
            '{' | '}' => index += 1,
            char => {
                // Style tags keep their content; `<sp>`, `<alt>` and `<v>`
                // are typography/verbatim TeX, not sung words.
                if stack.iter().all(|tag| {
                    matches!(
                        tag.as_str(),
                        "b" | "c" | "i" | "sc" | "tt" | "ul" | "e" | "eu" | "nlba"
                    )
                }) {
                    out.push(char);
                }
                index += 1;
            }
        }
    }
    out.trim().to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typography_is_not_a_lyric() {
        assert_eq!(plain_lyric("<sc>tus</sc>"), "tus");
        assert_eq!(plain_lyric("{DE}us"), "DEus");
        assert_eq!(plain_lyric("$(a)"), "(a)");
        assert_eq!(plain_lyric("<sp>V/</sp>."), ".");
    }

    #[test]
    fn segment_ticks_are_absolute_and_measures_do_not_collapse() {
        use scoretk_core::model::{Gregorian, Syllable};
        let mut gregorian = Gregorian::default();
        for text in ["DE", "us"] {
            gregorian.syllables.push(Syllable {
                text: text.to_owned(),
                notes: vec![
                    scoretk_core::model::GregorianNote::default(),
                    scoretk_core::model::GregorianNote::default(),
                ],
                ..Syllable::default()
            });
        }
        gregorian.syllables.push(Syllable::default()); // bar-only group
        let mut score = Score::new();
        build_projection(&mut score, &gregorian);
        assert!(score.measures[1].start_tick > score.measures[0].start_tick);
        assert!(score.measures[2].len > scoretk_core::model::Fraction::ZERO);
        for measure in &score.measures {
            for segment in &measure.segments {
                assert!(segment.tick >= measure.start_tick, "absolute segment tick");
            }
        }
    }

    #[test]
    fn the_first_note_of_a_syllable_carries_the_word() {
        use scoretk_core::model::{Extension, Gregorian, GregorianHeader, GregorianNote, Syllable};
        let mut gregorian = Gregorian {
            headers: vec![GregorianHeader {
                name: "name".to_owned(),
                value: "Factus".to_owned(),
            }],
            ..Gregorian::default()
        };
        gregorian.syllables.push(Syllable {
            text: "<sc>Fac</sc>tus".to_owned(),
            notes: vec![
                GregorianNote {
                    letter: 'e',
                    ..GregorianNote::default()
                },
                GregorianNote {
                    letter: 'i',
                    ..GregorianNote::default()
                },
            ],
            ..Syllable::default()
        });
        let mut score = Score::new();
        build_projection(&mut score, &gregorian);
        score.extensions.format = Some(Extension::Gregorian(gregorian));
        let with_lyric = score.measures[0]
            .segments
            .iter()
            .filter_map(|segment| {
                match segment.element(scoretk_core::model::Track::new(
                    scoretk_core::model::StaffId::from_raw(0),
                    0,
                )) {
                    Some(SegmentItem::ChordRest(chord_rest)) => Some(chord_rest),
                    _ => None,
                }
            })
            .filter(|chord_rest| !chord_rest.lyrics.is_empty())
            .count();
        assert_eq!(with_lyric, 1, "one syllable, one lyric on its first note");
    }
}
