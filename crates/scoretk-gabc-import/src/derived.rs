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
//! (`Gabc.tex:801-845`): the one-quarter-per-letter spine is a deliberate
//! choice external to the format. The projection itself lives in
//! `scoretk_core::model::Gregorian::build_derived_score` so the MusicXML
//! reader can rebuild the identical view when it carries the L6 payload; this
//! wrapper only preserves the metadata gathered from the source headers.

use scoretk_core::model::{Gregorian, Score};

/// Replaces `score`'s event spine with the projection of `gregorian`.
pub(super) fn build_projection(score: &mut Score, gregorian: &Gregorian) {
    let metadata = score.metadata.clone();
    *score = gregorian.build_derived_score();
    score.metadata = metadata;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_ticks_are_absolute_and_bars_close_measures() {
        use scoretk_core::model::{BarKind, Gregorian, Syllable};
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
        gregorian.syllables.push(Syllable {
            bars: vec![BarKind::DivisioMinima],
            ..Syllable::default()
        });
        gregorian.syllables.push(Syllable {
            notes: vec![scoretk_core::model::GregorianNote::default()],
            bars: vec![BarKind::DivisioFinalis],
            ..Syllable::default()
        });
        let mut score = Score::new();
        build_projection(&mut score, &gregorian);
        assert_eq!(score.measures.len(), 2, "one measure per separation bar");
        assert_eq!(score.notes.len(), 5);
        assert!(score.measures[1].start_tick > score.measures[0].start_tick);
        for measure in &score.measures {
            assert!(measure.len > scoretk_core::model::Fraction::ZERO);
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
                    Some(scoretk_core::model::SegmentItem::ChordRest(chord_rest)) => {
                        Some(chord_rest)
                    }
                    _ => None,
                }
            })
            .filter(|chord_rest| !chord_rest.lyrics.is_empty())
            .count();
        assert_eq!(with_lyric, 1, "one syllable, one lyric on its first note");
    }

    #[test]
    fn source_metadata_survives_the_projection() {
        let mut score = Score::new();
        score.metadata.work_title = "Factus est".to_owned();
        let gregorian = Gregorian::default();
        build_projection(&mut score, &gregorian);
        assert_eq!(score.metadata.work_title, "Factus est");
    }
}
