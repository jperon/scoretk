// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Allongement des accords dans une mesure 6/8 : `01_01_Etudiants`.
//!
//! Depuis l'extension « MIDI mecanique », ce cas synthetique (attaques et
//! fins sur la grille) garde ses durees ecrites. La voie MuseScore 4.7.4
//! reste active pour les entrees non mecaniques (second test).
//!
//! La piste 4 du MIDI porte un accord de 0,5 noire à 1,5, puis un autre à 3,0,
//! etc. MuseScore 4.7.4 les allonge à 1,5 noire, jusqu'au temps suivant de la
//! mesure 6/8 (3/8 de ronde). `beatLength(6/8)` vaut 3/8 de ronde : le temps
//! est à 1,5 noire, et `minimizeNumberOfRests`/`lengthenNote` remplissent le
//! silence jusqu'à 3,0. Le port utilisait la mesure réduite (3/4), dont le
//! temps vaut 1 noire, et s'arrêtait à 2,0 : aucun allongement.

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
fn accords_de_six_huit_allonges_jusqu_au_temps_suivant() {
    // 6/8 : le 2e champ de `Ev::TimeSig` est la puissance de deux du dénominateur.
    let mut events = vec![(0, Ev::TimeSig(6, 3))];
    // Trois accords de 0,5 noire aux temps 1,5, 3,0 et 4,5, plus un accord à 6,0.
    for (on, pitches) in [
        (1.5, [50u8, 53, 57]),
        (3.0, [45, 52, 57]),
        (4.5, [50, 53, 57]),
        (6.0, [45, 52, 57]),
    ] {
        for pitch in pitches {
            events.push(((on * 480.0) as u32, Ev::NoteOn(0, pitch, 90)));
            events.push((((on + 0.5) * 480.0) as u32, Ev::NoteOff(0, pitch)));
        }
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let got = durations(&score);
    // 1,5 / 3,0 / 4,5 / 6,0 noires → 3/8, 3/4, 9/8, 3/2 (unités : ronde) ;
    // chaque accord dure 1,5 noire = 3/8 de ronde.
    // MIDI mecanique : les durees ecrites (0,5 noire = 1/8 de ronde) sont
    // conservees au lieu d'etre allongees par `lengthenNote`.
    let expected: Vec<_> = [
        (Fraction::new(3, 8), [50u8, 53, 57]),
        (Fraction::new(3, 4), [45, 52, 57]),
        (Fraction::new(9, 8), [50, 53, 57]),
        (Fraction::new(3, 2), [45, 52, 57]),
    ]
    .into_iter()
    .flat_map(|(tick, pitches)| {
        pitches
            .into_iter()
            .map(move |pitch| (tick, pitch, Fraction::new(1, 8)))
    })
    .collect();
    assert_eq!(got, expected, "durees ecrites conservees (MIDI mecanique)");
}

/// Les memes accords avec des attaques legerement hors grille sont traites
/// comme une performance humaine : la voie MuseScore 4.7.4 reste active.
#[test]
fn accords_de_six_huit_humains_sont_allonges() {
    let mut events = vec![(0, Ev::TimeSig(6, 3))];
    for (on, pitches) in [
        (1.5, [50u8, 53, 57]),
        (3.0, [45, 52, 57]),
        (4.5, [50, 53, 57]),
        (6.0, [45, 52, 57]),
    ] {
        for pitch in pitches {
            let shift = u32::from(pitch % 2 == 0) * 7;
            events.push((((on * 480.0) as u32) + shift, Ev::NoteOn(0, pitch, 90)));
            events.push(((((on + 0.5) * 480.0) as u32) + shift, Ev::NoteOff(0, pitch)));
        }
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let got = durations(&score);
    assert!(
        got.iter()
            .any(|(_, _, duration)| *duration == Fraction::new(3, 8)),
        "la voie humaine doit allonger au moins un accord : {got:?}"
    );
}
