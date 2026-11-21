// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Découpage des notes aux temps (`MTrack::processPendingNotes`).
//!
//! MuseScore coupe une note qui franchit un temps en fragments liés :
//! `processPendingNotes` (`importmidi.cpp:553-611`) prend la **première**
//! figure de `toDurationList`, pose `chord->setTicks(d.fraction())` puis
//! `setTies` sur la continuation. Une noire pointée de 3 noires commençant sur
//! la deuxième croche ne doit donc pas sortir d'un seul tenant.
mod common;
use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

fn pitch_segments(score: &Score, pitch: u8) -> Vec<(Fraction, Fraction)> {
    let mut out = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(cr) = item {
                    if let ChordRestKind::Chord(chord) = &cr.kind {
                        if chord
                            .notes
                            .iter()
                            .any(|id| score.notes[id.index()].pitch == pitch)
                        {
                            out.push((
                                segment.tick,
                                cr.ticks.unwrap_or_else(|| cr.duration.ticks()),
                            ));
                        }
                    }
                }
            }
        }
    }
    out.sort_by_key(|(tick, _)| *tick);
    out
}

#[test]
fn note_franchissant_un_temps_est_liee() {
    let mut events = vec![(0, Ev::TimeSig(4, 2)), (0, Ev::Tempo(500_000))];
    // Fa4 de 0,5 à 3,5 noires (3 noires, à cheval sur les temps 2 et 3).
    events.push((240, Ev::NoteOn(0, 65, 90)));
    events.push((1680, Ev::NoteOff(0, 65)));
    events.push((1920, Ev::NoteOn(0, 67, 90)));
    events.push((2400, Ev::NoteOff(0, 67)));
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let segments = pitch_segments(&score, 65);
    assert!(
        segments.len() >= 2,
        "la note doit être découpée en fragments liés : {segments:?}"
    );
    assert_eq!(
        segments[0].0,
        Fraction::new(1, 8),
        "premier fragment à la croche"
    );
    assert!(
        segments[0].1 < Fraction::new(3, 4),
        "le premier fragment ne fait pas les 3 noires : {segments:?}"
    );
}
