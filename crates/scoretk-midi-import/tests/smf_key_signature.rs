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

//! Drapeau mineur du méta `FF 59 02 sf mi` (SMF §Meta-Events).
//!
//! Le mode est lu dans le modèle, comme pour l'import MusicXML, et une seconde
//! armure en cours de morceau doit pouvoir revenir au mode majeur. La deuxième
//! portée, sans armure explicite, sert de cas « non concerné ».

mod common;

use scoretk_core::model::{
    ChordRestKind, Fraction, KeyMode, Score, SegmentItem, SegmentType, StaffId,
};
use scoretk_midi_import::read_midi;

use common::{smf, Ev};

/// Notes `(hauteur, attaque, durée)` en fractions de ronde, toutes portées.
fn notes(score: &Score) -> Vec<(u8, Fraction, Fraction)> {
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
                let duration = chord_rest
                    .ticks
                    .unwrap_or_else(|| chord_rest.duration.ticks());
                for note_id in &chord.notes {
                    result.push((score.notes[note_id.index()].pitch, segment.tick, duration));
                }
            }
        }
    }
    result.sort_by_key(|(pitch, tick, _)| (*tick, *pitch));
    result
}

/// Armures `(tick, altérations, mode)` d'une portée.
fn key_sigs(score: &Score, staff_index: usize) -> Vec<(Fraction, i8, KeyMode)> {
    let staff = StaffId::from_index(staff_index);
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if segment.kind != SegmentType::KeySig {
                continue;
            }
            for (track, item) in segment.iter_elements() {
                if track.staff() != staff {
                    continue;
                }
                let SegmentItem::KeySig(key_sig) = item else {
                    continue;
                };
                result.push((segment.tick, key_sig.concert_key.fifths(), key_sig.mode));
            }
        }
    }
    result.sort_by_key(|(tick, _, _)| *tick);
    result
}

#[test]
fn armure_mineure_lue_puis_changée_en_majeur() {
    // Deux portées, 4/4 à 480 ticks/noire : quatre blanches par portée sur deux
    // mesures. La portée 1 porte do mineur (3 bémols, mi=1), puis do majeur.
    let bytes = smf(&[
        vec![
            (0, Ev::Tempo(500_000)),
            (0, Ev::TimeSig(4, 2)),
            (0, Ev::KeySig(-3, true)),
            (0, Ev::NoteOn(0, 60, 90)),
            (960, Ev::NoteOff(0, 60)),
            (960, Ev::NoteOn(0, 63, 90)),
            (1920, Ev::NoteOff(0, 63)),
            (1920, Ev::KeySig(0, false)),
            (1920, Ev::NoteOn(0, 67, 90)),
            (2880, Ev::NoteOff(0, 67)),
            (2880, Ev::NoteOn(0, 72, 90)),
            (3840, Ev::NoteOff(0, 72)),
        ],
        vec![
            (0, Ev::NoteOn(0, 48, 80)),
            (1920, Ev::NoteOff(0, 48)),
            (1920, Ev::NoteOn(0, 50, 80)),
            (3840, Ev::NoteOff(0, 50)),
        ],
    ]);

    let score = read_midi(&bytes).expect("lecture du SMF");
    assert_eq!(score.staves.len(), 2, "portées");
    assert_eq!(score.measures.len(), 2, "mesures");
    assert_eq!(score.notes.len(), 6, "notes");

    let expected = vec![
        (48, Fraction::ZERO, Fraction::from_integer(1)),
        (60, Fraction::ZERO, Fraction::HALF),
        (63, Fraction::HALF, Fraction::HALF),
        (50, Fraction::from_integer(1), Fraction::from_integer(1)),
        (67, Fraction::from_integer(1), Fraction::HALF),
        (72, Fraction::new(3, 2), Fraction::HALF),
    ];
    assert_eq!(notes(&score), expected, "hauteurs, attaques et durées");

    assert_eq!(
        key_sigs(&score, 0),
        vec![
            (Fraction::ZERO, -3, KeyMode::Minor),
            (Fraction::from_integer(1), 0, KeyMode::Major),
        ],
        "le drapeau mi doit suivre le méta-event FF 59",
    );
    // L'armure explicite est propagée aux portées qui n'en ont pas.
    assert_eq!(key_sigs(&score, 1), key_sigs(&score, 0), "portée 2");

    // Cas non concerné : sans méta FF 59, l'armure déduite garde son mode
    // inconnu, exactement comme avant la correction.
    let plain = smf(&[vec![
        (0, Ev::NoteOn(0, 60, 90)),
        (960, Ev::NoteOff(0, 60)),
        (960, Ev::NoteOn(0, 64, 90)),
        (1920, Ev::NoteOff(0, 64)),
    ]]);
    let plain = read_midi(&plain).expect("lecture sans armure");
    let detected = key_sigs(&plain, 0);
    assert!(
        detected.is_empty(),
        "aucune armure explicite sans méta FF 59"
    );
}
