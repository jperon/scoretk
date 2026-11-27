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

//! Extension additive « fidélité SMF » : le tick source est conservé.

mod common;

use scoretk_core::model::{Annotation, ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::{read_midi, read_midi_with, ImportOptions};

use common::{smf, Ev};

fn onsets(score: &Score) -> Vec<Fraction> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(chord_rest) = item {
                    if matches!(chord_rest.kind, ChordRestKind::Chord(_)) {
                        result.push(segment.tick);
                    }
                }
            }
        }
    }
    result.sort();
    result
}

/// Four attacks at ticks 0/100/200/300 of a 480 PPQ file. A regular tuplet
/// is already recovered by MuseScore's detector; an irregular grid is not:
/// the default 4.7.4 import snaps those onsets, `smf_exact_ticks` keeps the
/// source ticks (SMF stores only tick arithmetic, `format-ref/midi.md` §
/// "Instruments, parts, repeats").
#[test]
fn exact_ticks_recover_an_irregular_onset_grid() {
    let bytes = smf(&[vec![
        (0, Ev::TimeSig(4, 2)),
        (0, Ev::Tempo(500_000)),
        (0, Ev::Program(0, 0)),
        (0, Ev::NoteOn(0, 60, 80)),
        (100, Ev::NoteOn(0, 62, 80)),
        (200, Ev::NoteOn(0, 64, 80)),
        (300, Ev::NoteOn(0, 67, 80)),
        (100, Ev::NoteOff(0, 60)),
        (200, Ev::NoteOff(0, 62)),
        (300, Ev::NoteOff(0, 64)),
        (400, Ev::NoteOff(0, 67)),
    ]]);
    let expected: Vec<Fraction> = [0, 100, 200, 300]
        .iter()
        .map(|tick| Fraction::from_ticks(*tick, 1920))
        .collect();

    let default_score = read_midi(&bytes).expect("default import");
    let default_onsets = onsets(&default_score);
    assert!(
        !expected.iter().all(|tick| default_onsets.contains(tick)),
        "the MuseScore default must keep snapping to the sixteenth grid: {default_onsets:?}"
    );

    let options = ImportOptions {
        smf_exact_ticks: true,
        ..Default::default()
    };
    let exact_score = read_midi_with(&bytes, &options).expect("exact import");
    let exact_onsets = onsets(&exact_score);
    // A written 1/12-whole note cannot be one binary duration: `processPendingNotes`
    // chains tied chord rests. Every attack tick must still be present.
    assert!(
        expected.iter().all(|tick| exact_onsets.contains(tick)),
        "exact mode must preserve the source attack ticks: {exact_onsets:?}"
    );
}

fn tempos(score: &Score) -> Vec<f64> {
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for annotation in &segment.annotations {
                if let Annotation::Tempo(tempo) = &annotation.annotation {
                    if let Some(bps) = tempo.beats_per_second {
                        result.push(bps);
                    }
                }
            }
        }
    }
    result
}

/// `FF 51 03 tttttt` at the last meta tick is a tempo change the file carries.
/// `[SMF §Meta-Events]` places no restriction on its tick; exact mode adds one
/// measure so the tempo can be anchored at that tick rather than dropped.
#[test]
fn exact_ticks_keep_a_tempo_change_at_the_last_meta_tick() {
    let bytes = smf(&[vec![
        (0, Ev::TimeSig(4, 2)),
        (0, Ev::Tempo(500_000)),
        (0, Ev::NoteOn(0, 60, 80)),
        (480, Ev::NoteOff(0, 60)),
        (1920, Ev::Tempo(652_175)),
    ]]);
    let options = ImportOptions {
        smf_exact_ticks: true,
        ..Default::default()
    };
    let default_tempos = tempos(&read_midi(&bytes).expect("default import"));
    assert!(!default_tempos
        .iter()
        .any(|bps| (bps - 92.0 / 60.0).abs() < 1e-9));
    let exact_tempos = tempos(&read_midi_with(&bytes, &options).expect("exact import"));
    assert!(
        exact_tempos
            .iter()
            .any(|bps| (bps - 92.0 / 60.0).abs() < 1e-9),
        "the FF 51 at the last tick must survive: {exact_tempos:?}"
    );
}
