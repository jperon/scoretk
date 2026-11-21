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
                                cr.duration.ticks(),
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
    // Évènements exacts de `tuplet_off_time_other_bar2.mid` (384 ticks/noire,
    // 4/4) : la réduction précédente arrondissait les attaques et masquait
    // l'écart d'une demi-croche entre les deux pipelines.
    // `Ev::TimeSig` prend l'exposant du dénominateur SMF : 2 = 4/4.
    let mut events = vec![(0, Ev::TimeSig(4, 2)), (0, Ev::Tempo(500_000))];
    for (tick, pitch, on) in [
        (0, 72, true),
        (768, 72, false),
        (769, 74, true),
        (1024, 67, true),
        (1028, 74, false),
        (1280, 67, false),
        (1281, 69, true),
        (1788, 69, false),
        (1793, 74, true),
        (2048, 74, false),
        (2048, 67, true),
        (2304, 67, false),
    ] {
        events.push((
            tick,
            if on {
                Ev::NoteOn(0, pitch, 80)
            } else {
                Ev::NoteOff(0, pitch)
            },
        ));
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(384)), &[events]);
    let options = ImportOptions {
        simplify_durations: false,
        max_voice_count: VoiceCount::V1,
        ..ImportOptions::default()
    };
    let score = read_midi_with(&bytes, &options).expect("lecture");
    let got = durations(&score);
    // Le n-olet parasite allongeait la note 74 de 1,0 à 1,25 noire (5/16 de
    // ronde). Avec l'insertion précoce, le modèle stocke la durée sonore
    // (1/4 de ronde) ; l'export MusicXML la note 3/16 avec le ratio 3:2,
    // comme la référence (vérifié par l'oracle sur le fichier complet).
    let note = got
        .iter()
        .find(|(tick, pitch, _)| *tick == Fraction::new(1, 2) && *pitch == 74)
        .expect("note 74 à 2,0 noires");
    assert_eq!(
        note.2,
        Fraction::new(1, 4),
        "durée sonore de la note 74 : {got:?}"
    );
    assert!(
        !got.iter()
            .any(|(tick, pitch, len)| *tick == Fraction::new(1, 2)
                && *pitch == 74
                && *len > Fraction::new(1, 4)),
        "la note ne doit plus être allongée par un triolet : {got:?}"
    );
}
