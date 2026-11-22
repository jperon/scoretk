// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon

//! `splitFirstTupletChords` : deux n-olets qui partagent leur premier accord.
//!
//! Reproduit `tuplet_2_voices_3_5_tuplets` (test amont de MuseScore) : un
//! triolet (3:2) et un quintuplet (5:4) commencent sur le **même** accord de
//! deux notes (64 et 67, canal 0). `MChord::collectChords` en fait un seul
//! accord ; `MidiTuplet::splitFirstTupletChords`
//! (`importmidi_tuplet.cpp:401-440`) le scinde en deux accords à la même date :
//! le n-olet **antérieur** dans le vecteur garde la première note (la plus
//! grave, 64), le suivant garde l'autre (67). La référence MuseScore 4.7.4
//! place alors 67 (triolet) en voix 1 et 64 (quintuplet) en voix 2
//! (`sortVoicesByPitch`, `importmidi_voice.cpp:985-1029`).
//!
//! Test réduit : les mêmes attaques, avec les mêmes durées, dans un SMF
//! construit en mémoire.

mod common;

use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Score, SegmentItem};

const TPB: f64 = 384.0;

/// Voix (indice de piste du segment) de la note `pitch` au tick `tick`.
fn voix_de(score: &Score, tick: f64, pitch: u8) -> Option<(usize, u32)> {
    for measure in &score.measures {
        for segment in &measure.segments {
            let position = segment.tick.as_f64() * 4.0;
            if (position - tick).abs() > 1e-6 {
                continue;
            }
            for (track, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    if score.note(*id).pitch == pitch {
                        return Some((track.voice() as usize, track.staff().raw()));
                    }
                }
            }
        }
    }
    None
}

#[test]
fn deux_n_olets_partagent_le_premier_accord() {
    // (tick, pitch, off_tick), tpb = 384 : 3:2 sur 0, 1/3, 2/3 de noire ;
    // 5:4 sur 0, 1/5, 2/5, 3/5, 4/5.
    let notes: [(u32, u8, u32); 10] = [
        (0, 64, 74),     // quintuplet, première note (la plus grave)
        (0, 67, 125),    // triolet, première note
        (77, 62, 150),   // quintuplet
        (128, 71, 252),  // triolet
        (154, 60, 229),  // quintuplet
        (231, 62, 305),  // quintuplet
        (256, 72, 381),  // triolet
        (308, 59, 380),  // quintuplet
        (385, 65, 750),  // noire suivante
        (769, 64, 1140), // noire suivante
    ];
    let mut events = vec![(0, Ev::TimeSig(4, 4)), (0, Ev::Tempo(500_000))];
    for (on, pitch, off) in notes {
        events.push((on, Ev::NoteOn(0, pitch, 80)));
        events.push((off, Ev::NoteOff(0, pitch)));
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(
        Format::Parallel,
        Timing::Metrical(u15::new(TPB as u16)),
        &[events],
    );
    let options = scoretk_midi_import::ImportOptions {
        tuplet_notation: true,
        quant_value: scoretk_midi_import::QuantValue::Q32,
        ..scoretk_midi_import::ImportOptions::default()
    };
    let score = scoretk_midi_import::read_midi_with(&bytes, &options).expect("lecture");

    let triolet = voix_de(&score, 0.0, 67);
    let quintuplet = voix_de(&score, 0.0, 64);
    assert!(triolet.is_some(), "67 absent au tick 0");
    assert!(quintuplet.is_some(), "64 absent au tick 0");
    let (voix_triolet, staff_triolet) = triolet.expect("triolet");
    let (voix_quintuplet, staff_quintuplet) = quintuplet.expect("quintuplet");
    assert_eq!(
        staff_triolet, staff_quintuplet,
        "les deux voix doivent rester sur la même portée"
    );
    // Triolet (hauteur moyenne la plus haute) en voix 1, quintuplet en voix 2.
    assert_eq!(
        (voix_triolet, voix_quintuplet),
        (0, 1),
        "voix du triolet puis du quintuplet"
    );
}
