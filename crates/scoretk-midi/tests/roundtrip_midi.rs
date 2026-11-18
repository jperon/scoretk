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

//! Aller-retour `.mscx` → MIDI → modèle → MIDI.
//!
//! Pour chaque source du corpus MIDI, le test exporte la partition en MIDI,
//! relit ce MIDI avec [`scoretk_midi::read_midi`], le réexporte, puis compare
//! les deux fichiers MIDI : mêmes hauteurs, mêmes attaques, et durées à la
//! quantification près (grille de double croche de l'import).
//!
//! C'est la garantie que la lecture ne perd ni ne déplace de note ; les durées
//! peuvent changer légèrement, puisque la notation quantifie et simplifie.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

/// Tolérance de durée, en ticks MIDI (double croche = 120 ticks à 480/noire).
const DURATION_TOLERANCE: i64 = 120;

/// Fichiers dont l'aller-retour ne doit pas régresser (notes et attaques
/// identiques, durées à ±[`DURATION_TOLERANCE`] ticks).
const EXPECTED_ROUNDTRIP: &[&str] = &[
    "v3/copypaste_tuplet_01",
    "v3/grace",
    "v3/repeat01",
    "v3/repeat06",
    "v3/repeat07",
    "v3/repeat11",
    "v3/repeat20",
    "v3/repeat39",
    "v3/testTuplets8",
    "v4/dynamics",
    "v4/hairpin_simple",
    "v4/hairpins_and_repeats",
    "v4/linked-ties",
    "v4/mmrest",
    "v4/repeat65",
    "v4/repeat66",
    "v4/repeat68",
    "v4/repeat69",
    "v4/tenuto_accent",
    "v4/testTempoPrecision",
    "v4/timesig-09",
];

fn corpus(version: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
        .join(name)
}

/// Note d'un fichier MIDI, sans piste : hauteur, attaque et durée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NoteEvent {
    pitch: u8,
    on: i64,
    len: i64,
}

/// Extrait les notes d'un SMF, triées (`pitch`, `on`, `len`).
fn midi_notes(bytes: &[u8]) -> Vec<NoteEvent> {
    let smf = midly::Smf::parse(bytes).expect("SMF valide");
    let mut notes = Vec::new();
    for track in &smf.tracks {
        let mut tick: i64 = 0;
        let mut active: BTreeMap<u8, i64> = BTreeMap::new();
        for event in track {
            tick += i64::from(event.delta.as_int());
            let (pitch, velocity) = match event.kind {
                midly::TrackEventKind::Midi { message, .. } => match message {
                    midly::MidiMessage::NoteOn { key, vel } => (key.as_int(), vel.as_int()),
                    midly::MidiMessage::NoteOff { key, .. } => (key.as_int(), 0),
                    _ => continue,
                },
                _ => continue,
            };
            if velocity > 0 {
                active.insert(pitch, tick);
            } else if let Some(on) = active.remove(&pitch) {
                notes.push(NoteEvent {
                    pitch,
                    on,
                    len: (tick - on).max(1),
                });
            }
        }
        for (pitch, on) in active {
            notes.push(NoteEvent { pitch, on, len: 1 });
        }
    }
    notes.sort();
    notes
}

/// Vrai si deux listes de notes coïncident à la quantification près.
///
/// Les notes sont regroupées par hauteur : l'import peut déplacer une attaque
/// de moins d'une double croche (grille de quantification) et allonger ou
/// raccourcir une durée de la même marge.
fn note_mismatches(left: &[NoteEvent], right: &[NoteEvent]) -> usize {
    if left.len() != right.len() {
        return left.len().abs_diff(right.len()) + left.len().min(right.len());
    }
    let mut by_pitch: BTreeMap<u8, (Vec<NoteEvent>, Vec<NoteEvent>)> = BTreeMap::new();
    for note in left {
        by_pitch.entry(note.pitch).or_default().0.push(*note);
    }
    for note in right {
        by_pitch.entry(note.pitch).or_default().1.push(*note);
    }
    let mut mismatches = 0;
    for (_, (mut expected, mut actual)) in by_pitch {
        expected.sort();
        actual.sort();
        if expected.len() != actual.len() {
            mismatches += expected.len().abs_diff(actual.len());
            continue;
        }
        for (expected, actual) in expected.iter().zip(actual.iter()) {
            // Une note plus courte que la grille d'import peut être allongée
            // pour remplir sa case (n-olet piqué, `applyTupletStaccato`) : sa
            // durée de départ n'est alors qu'une borne inférieure.
            let short_note = expected.len <= DURATION_TOLERANCE;
            let duration_ok = (expected.len - actual.len).abs() <= DURATION_TOLERANCE
                || (short_note && actual.len >= expected.len);
            if (expected.on - actual.on).abs() > DURATION_TOLERANCE || !duration_ok {
                mismatches += 1;
            }
        }
    }
    mismatches
}

fn original_sample(version: &str, name: &str) -> Vec<(NoteEvent, NoteEvent)> {
    let mscx = std::fs::read(corpus(version, &format!("{name}.mscx"))).expect("source .mscx");
    let container = MsczContainer::from_bytes(&mscx).expect("conteneur");
    let score: Score =
        scoretk_musescore_import::read::read_score(&container).expect("lecture .mscx");
    let first = scoretk_midi::write_midi(&score).expect("écriture MIDI");
    let reread = scoretk_midi::read_midi(&first).expect("lecture MIDI");
    let second = scoretk_midi::write_midi(&reread).expect("réécriture MIDI");
    let original = midi_notes(&first);
    let rewritten = midi_notes(&second);
    original
        .iter()
        .zip(rewritten.iter())
        .filter(|(left, right)| {
            left.pitch != right.pitch
                || (left.on - right.on).abs() > DURATION_TOLERANCE
                || (left.len - right.len).abs() > DURATION_TOLERANCE
        })
        .take(6)
        .map(|(left, right)| (*left, *right))
        .collect()
}

fn roundtrip(version: &str, name: &str) -> (bool, usize, usize) {
    let mscx = std::fs::read(corpus(version, &format!("{name}.mscx"))).expect("source .mscx");
    let container = MsczContainer::from_bytes(&mscx).expect("conteneur");
    let score: Score =
        scoretk_musescore_import::read::read_score(&container).expect("lecture .mscx");
    let first = scoretk_midi::write_midi(&score).expect("écriture MIDI");
    let reread = scoretk_midi::read_midi(&first).expect("lecture MIDI");
    let second = scoretk_midi::write_midi(&reread).expect("réécriture MIDI");

    let original = midi_notes(&first);
    let rewritten = midi_notes(&second);
    let mismatches = note_mismatches(&original, &rewritten);
    (mismatches == 0, original.len(), mismatches)
}

#[test]
fn aller_retour_midi() {
    let mut identical = 0;
    let mut failures = Vec::new();
    let mut details = String::new();
    for entry in EXPECTED_ROUNDTRIP {
        let (version, name) = entry.split_once('/').expect("version/nom");
        let (same, count, mismatches) = roundtrip(version, name);
        if same {
            identical += 1;
            println!("  {entry:<28} identique ({count} notes)");
        } else {
            println!("  {entry:<28} {mismatches} notes d'écart sur {count}");
            failures.push(*entry);
            let mut sample = String::new();
            for (left, right) in original_sample(version, name) {
                sample.push_str(&format!("  {left:?} -> {right:?}\n"));
            }
            details.push_str(&format!(
                "\n--- {entry} ---\n{mismatches} notes d'écart sur {count}\n{sample}"
            ));
        }
    }
    println!(
        "Aller-retour MIDI : {identical}/{} identiques",
        EXPECTED_ROUNDTRIP.len()
    );
    assert!(
        failures.is_empty(),
        "régression d'aller-retour MIDI sur : {}\n{}",
        failures.join(", "),
        details
    );
}
