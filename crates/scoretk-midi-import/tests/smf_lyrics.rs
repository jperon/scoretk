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

//! Conventions RP-017 des méta-évènements `FF 05` : syllabation, mélisme
//! (évènement vide) et drapeau « on » au tick 0. Vérifié dans deux mesures de
//! musique ordinaire, avec les hauteurs et les durées, pas seulement les
//! paroles.

mod common;

use scoretk_core::model::{
    ChordRestKind, Fraction, LyricsSyllabic, Score, SegmentItem,
};
use scoretk_midi_import::read_midi;

use common::{Ev, smf};

type Lyric = (Fraction, String, LyricsSyllabic, Option<Fraction>);

fn lyrics(score: &Score) -> Vec<Lyric> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                for lyric in &chord_rest.lyrics {
                    result.push((
                        segment.tick,
                        lyric.text.clone(),
                        lyric.syllabic,
                        lyric.melisma,
                    ));
                }
            }
        }
    }
    result.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    result
}

fn notes(score: &Score) -> Vec<(u8, Fraction)> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for note_id in &chord.notes {
                    result.push((score.notes[note_id.index()].pitch, segment.tick));
                }
            }
        }
    }
    result.sort_by_key(|(pitch, tick)| (*tick, *pitch));
    result
}

#[test]
fn paroles_rp017_syllabes_melismes_et_drapeau_zero() {
    let pitches = [60, 62, 64, 65, 67, 69, 71, 72];
    let mut track = vec![(0, Ev::Tempo(500_000)), (0, Ev::TimeSig(4, 2))];
    for (index, pitch) in pitches.iter().enumerate() {
        let tick = index as u32 * 480;
        track.push((tick, Ev::NoteOn(0, *pitch, 90)));
        track.push((tick + 480, Ev::NoteOff(0, *pitch)));
    }
    // Drapeau « on » au tick 0, mot « La-la-la », syllabe isolée, mot « Do-re »
    // dont la première syllabe est prolongée par un mélisme d'une noire.
    track.extend([
        (0, Ev::Lyric(b"")),
        (0, Ev::Lyric(b"La-")),
        (480, Ev::Lyric(b"la-")),
        (960, Ev::Lyric(b"la ")),
        (1440, Ev::Lyric(b"Sol")),
        (1920, Ev::Lyric(b"Do-")),
        (2400, Ev::Lyric(b"")),
        (2880, Ev::Lyric(b"re")),
        (3360, Ev::Lyric(b"Plain")),
    ]);

    // Le constructeur SMF encode des deltas : les évènements doivent être
    // donnés par tick croissant (l'ordre à tick égal est conservé).
    track.sort_by_key(|(tick, _)| *tick);
    let score = read_midi(&smf(&[track])).expect("lecture des paroles");
    assert_eq!(score.measures.len(), 2, "mesures");
    assert_eq!(
        notes(&score),
        pitches
            .iter()
            .enumerate()
            .map(|(i, pitch)| (*pitch, Fraction::new(i as i64, 4)))
            .collect::<Vec<_>>(),
        "hauteurs et attaques",
    );
    assert_eq!(
        lyrics(&score),
        vec![
            (Fraction::ZERO, "La".to_owned(), LyricsSyllabic::Begin, None),
            (
                Fraction::new(1, 4),
                "la".to_owned(),
                LyricsSyllabic::Middle,
                None
            ),
            (
                Fraction::HALF,
                "la".to_owned(),
                LyricsSyllabic::End,
                None
            ),
            (
                Fraction::new(3, 4),
                "Sol".to_owned(),
                LyricsSyllabic::Single,
                None
            ),
            (
                Fraction::from_integer(1),
                "Do".to_owned(),
                LyricsSyllabic::Begin,
                Some(Fraction::QUARTER)
            ),
            (
                Fraction::new(3, 2),
                "re".to_owned(),
                LyricsSyllabic::End,
                None
            ),
            (
                Fraction::new(7, 4),
                "Plain".to_owned(),
                LyricsSyllabic::Single,
                None
            ),
        ],
        "syllabation, mélisme et drapeau du tick 0",
    );
}
