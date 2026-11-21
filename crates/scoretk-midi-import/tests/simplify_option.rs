// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Option `simplifyDurations` de l'import MIDI (`MidiOperations`).
//!
//! Les tests amont de MuseScore compilés avec `dontSimplify` posent
//! `simplifyDurations = false` (`midiimport_tests.cpp:86`) : leurs `.mscx`
//! attendus décrivent un import sans allongement des notes. La CLI expose la
//! même option (`--midi-no-simplify`) pour que l'oracle puisse rejouer cette
//! configuration.

mod common;
use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::{read_midi, read_midi_with, ImportOptions};

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

fn six_huit() -> Vec<u8> {
    let mut events = vec![(0, Ev::TimeSig(6, 3))]; // 6/8
    for (on, pitches) in [(1.5, [50u8, 53, 57]), (3.0, [45, 52, 57])] {
        for pitch in pitches {
            events.push(((on * 480.0) as u32, Ev::NoteOn(0, pitch, 90)));
            events.push((((on + 0.5) * 480.0) as u32, Ev::NoteOff(0, pitch)));
        }
    }
    events.sort_by_key(|(tick, _)| *tick);
    smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events])
}

#[test]
fn sans_simplification_les_durees_restent_quantifiees() {
    let bytes = six_huit();

    // Par défaut (`simplifyDurations = true`) : accords allongés à 1,5 noire.
    let score = read_midi(&bytes).expect("lecture");
    let got = durations(&score);
    assert_eq!(got[0].2, Fraction::new(3, 8), "6/8 allongé : {got:?}");
    assert_eq!(got[3].2, Fraction::new(3, 8), "6/8 allongé : {got:?}");

    // Option désactivée : les durées du MIDI restent 0,5 noire.
    let options = ImportOptions {
        simplify_durations: false,
        ..ImportOptions::default()
    };
    let score = read_midi_with(&bytes, &options).expect("lecture");
    let got = durations(&score);
    assert_eq!(
        got[0].2,
        Fraction::new(1, 8),
        "sans simplification : {got:?}"
    );
    assert_eq!(
        got[3].2,
        Fraction::new(1, 8),
        "sans simplification : {got:?}"
    );
}
