// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! `tuplet_off_time_other_bar2` : triolet parasite rejeté, durées de MuseScore.
//!
//! Le test amont importe ce fichier avec `dontSimplify` et `maxVoiceCount = V_1`
//! (`midiimport_tests.cpp:79-92`). Les attaques tombent sur une grille de
//! triolet (0, 2/3, 4/3) ; l'ancien détecteur simplifié retenait ce triolet et
//! `toDurationList` rendait des durées de 2/3 de noire. Le filtre littéral
//! (`filterTuplets`) l'écarte : les durées attendues sont celles de la
//! référence MuseScore, 0,75 / 0,75 / 1,25 / 0,75 / 0,75.
mod common;
use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::{read_midi_with, ImportOptions, VoiceCount};

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
fn triolet_parasite_rejete_durees_de_la_reference() {
    let mut events = vec![(0, Ev::TimeSig(4, 2)), (0, Ev::Tempo(500_000))];
    for (on, off, pitch) in [
        (0.0, 2.0, 72u8),
        (2.003, 2.677, 74),
        (2.667, 3.333, 67),
        (3.336, 4.656, 69),
        (4.669, 5.333, 74),
        (5.333, 6.0, 67),
    ] {
        events.push(((on * 480.0) as u32, Ev::NoteOn(0, pitch, 90)));
        events.push(((off * 480.0) as u32, Ev::NoteOff(0, pitch)));
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let options = ImportOptions {
        simplify_durations: false,
        max_voice_count: VoiceCount::V1,
        tuplet_notation: true,
        ..ImportOptions::default()
    };
    let score = read_midi_with(&bytes, &options).expect("lecture");
    let got = durations(&score);
    // Le n-olet parasite allongeait la note 74 de 2,0 à 3,25 (1,25 noire) ;
    // sans lui, son premier fragment dure 3/16 de ronde, comme la référence.
    let note = got
        .iter()
        .find(|(tick, pitch, _)| *tick == Fraction::new(1, 2) && *pitch == 74)
        .expect("note 74 à 2,0 noires");
    assert_eq!(
        note.2,
        Fraction::new(3, 16),
        "durée de la note 74 (0,75 noire) : {got:?}"
    );
    assert!(
        !got.iter()
            .any(|(tick, pitch, len)| *tick == Fraction::new(1, 2)
                && *pitch == 74
                && *len > Fraction::new(3, 16)),
        "la note ne doit plus être allongée par un triolet : {got:?}"
    );
}
