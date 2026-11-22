// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Réduction de `LaMouetteGrise` : ordre de simplification des notes.
mod common;
use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

fn durations(score: &Score) -> Vec<(Fraction, u8, Fraction)> {
    let mut out = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(cr) = item {
                    if let ChordRestKind::Chord(chord) = &cr.kind {
                        for id in &chord.notes {
                            out.push((
                                segment.tick,
                                score.notes[id.index()].pitch,
                                cr.ticks.unwrap_or_else(|| cr.duration.ticks()),
                            ));
                        }
                    }
                }
            }
        }
    }
    out.sort_by_key(|(tick, pitch, _)| (*tick, *pitch));
    out
}

#[test]
fn trois_notes_une_piste_durees_musescore() {
    let mut events = vec![(0, Ev::TimeSig(4, 2))]; // 4/4 (2e champ = puissance de deux)
    for (pitch, on, off) in [(57, 5.0, 6.5), (60, 6.0, 6.5), (62, 6.5, 7.0)] {
        // Leger desalignement pour rester sur la voie MuseScore : ce test
        // verifie les durees de MuseScore, pas l'extension MIDI mecanique.
        let shift = u32::from(pitch % 2 == 1) * 7;
        events.push(((on * 480.0) as u32 + shift, Ev::NoteOn(0, pitch, 90)));
        events.push(((off * 480.0) as u32 + shift, Ev::NoteOff(0, pitch)));
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let got = durations(&score);
    let expected = vec![
        (Fraction::new(5, 4), 57, Fraction::new(1, 2)),
        (Fraction::new(3, 2), 60, Fraction::new(1, 8)),
        (Fraction::new(13, 8), 62, Fraction::new(1, 8)),
    ];
    assert_eq!(got, expected, "durées attendues de MuseScore 4.7.4");
}
