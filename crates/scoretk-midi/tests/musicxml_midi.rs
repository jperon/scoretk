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

//! Chaîne MusicXML → modèle → MIDI.
//!
//! Pour chaque référence `tests/corpus/midi/{v3,v4}/X.ref.mid`, le test lit la
//! référence MusicXML correspondante (`tests/corpus/{v3,v4}/X.ref.musicxml`)
//! avec [`read_musicxml`], écrit le MIDI avec [`write_midi`] et compare notes
//! (piste, canal, hauteur, début, fin, vélocité) et tempos à la référence
//! 4.7.4, avec la même normalisation que `corpus_midi.rs`.
//!
//! C'est la vérification que les nuances et tempos lus depuis `<direction>`
//! produisent bien les vélocités et les changements de tempo de MuseScore.
//! Le test affiche « MusicXML → MIDI : N/M identiques » et n'échoue que sur
//! [`EXPECTED_IDENTICAL`], qui ne doit pas régresser.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

use scoretk_midi::write_midi;
use scoretk_musicxml::read_musicxml;

/// Fichiers dont la conversion MusicXML → MIDI est identique à la référence.
const EXPECTED_IDENTICAL: &[&str] = &[
    "v4/dynamics",
    "v4/hairpin_simple",
    "v4/linked-ties",
    "v4/mmrest",
    "v4/repeat68",
    "v4/repeat69",
    "v4/tenuto_accent",
];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NoteEvent {
    track: usize,
    channel: u8,
    key: u8,
    start: u32,
    end: u32,
    velocity: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MidiSummary {
    notes: Vec<NoteEvent>,
    tempos: Vec<(u32, u32)>,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .canonicalize()
        .expect("corpus introuvable")
}

/// Résume un fichier MIDI : notes appariées et changements de tempo.
fn summarize(bytes: &[u8], label: &str) -> MidiSummary {
    let smf = Smf::parse(bytes).unwrap_or_else(|error| panic!("{label} : MIDI invalide : {error}"));
    let mut notes = Vec::new();
    let mut tempos: BTreeMap<u32, u32> = BTreeMap::new();

    for (track_index, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0u32;
        let mut open: BTreeMap<(u8, u8), Vec<(u32, u8)>> = BTreeMap::new();
        for event in track {
            tick += event.delta.as_int();
            match event.kind {
                TrackEventKind::Midi { channel, message } => match message {
                    MidiMessage::NoteOn { key, vel } => {
                        let key = key.as_int();
                        let velocity = vel.as_int();
                        if velocity == 0 {
                            if let Some((start, note_velocity)) =
                                open.get_mut(&(channel.as_int(), key)).and_then(Vec::pop)
                            {
                                notes.push(NoteEvent {
                                    track: track_index,
                                    channel: channel.as_int(),
                                    key,
                                    start,
                                    end: tick,
                                    velocity: note_velocity,
                                });
                            }
                        } else {
                            open.entry((channel.as_int(), key))
                                .or_default()
                                .push((tick, velocity));
                        }
                    }
                    MidiMessage::NoteOff { key, .. } => {
                        if let Some((start, velocity)) = open
                            .get_mut(&(channel.as_int(), key.as_int()))
                            .and_then(Vec::pop)
                        {
                            notes.push(NoteEvent {
                                track: track_index,
                                channel: channel.as_int(),
                                key: key.as_int(),
                                start,
                                end: tick,
                                velocity,
                            });
                        }
                    }
                    _ => {}
                },
                TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                    tempos.insert(tick, value.as_int());
                }
                _ => {}
            }
        }
    }

    notes.sort();
    // Ne garder que les changements de tempo effectifs : MuseScore réécrit
    // parfois le tempo en vigueur à une frontière de reprise.
    let mut effective = Vec::new();
    for (tick, value) in tempos {
        if effective.last().map(|(_, previous)| *previous) != Some(value) {
            effective.push((tick, value));
        }
    }
    MidiSummary {
        notes,
        tempos: effective,
    }
}

fn compare(name: &str, source: &Path, reference: &Path) -> Result<(), Vec<String>> {
    let bytes = fs::read(source)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", source.display()));
    let score = read_musicxml(&bytes)
        .unwrap_or_else(|error| panic!("{name} : lecture MusicXML impossible : {error}"));
    let produced = write_midi(&score)
        .unwrap_or_else(|error| panic!("{name} : écriture MIDI impossible : {error}"));
    let reference_bytes = fs::read(reference)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", reference.display()));
    let ours = summarize(&produced, name);
    let theirs = summarize(&reference_bytes, name);

    let mut differences = Vec::new();
    if ours.notes != theirs.notes {
        differences.push(format!(
            "notes : {} produites, {} attendues",
            ours.notes.len(),
            theirs.notes.len()
        ));
        let mut shown = 0;
        for (index, (mine, expected)) in ours.notes.iter().zip(theirs.notes.iter()).enumerate() {
            if mine != expected {
                differences.push(format!("  note {index} : {mine:?} ≠ {expected:?}"));
                shown += 1;
                if shown == 8 {
                    break;
                }
            }
        }
        let extra = ours.notes.len().saturating_sub(theirs.notes.len());
        if shown == 0 && extra > 0 {
            for note in ours.notes.iter().skip(theirs.notes.len()).take(8) {
                differences.push(format!("  note en trop : {note:?}"));
            }
        }
    }
    if ours.tempos != theirs.tempos {
        differences.push(format!(
            "tempo : {:?} produits, {:?} attendus",
            ours.tempos, theirs.tempos
        ));
    }
    if differences.is_empty() {
        Ok(())
    } else {
        Err(differences)
    }
}

fn names(corpus: &Path) -> Vec<(String, String, String)> {
    let references = corpus.join("midi");
    let mut names = Vec::new();
    for version in ["v3", "v4"] {
        let directory = references.join(version);
        if !directory.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&directory).expect("références MIDI illisibles") {
            let path = entry.expect("entrée illisible").path();
            if path.extension().is_some_and(|extension| extension == "mid") {
                let stem = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .expect("nom de référence invalide")
                    .trim_end_matches(".ref")
                    .to_owned();
                let musicxml = corpus.join(version).join(format!("{stem}.ref.musicxml"));
                if musicxml.is_file() {
                    names.push((format!("{version}/{stem}"), version.to_owned(), stem));
                }
            }
        }
    }
    names.sort();
    names
}

#[test]
fn musicxml_midi() {
    let corpus = corpus_dir();
    let names = names(&corpus);

    let mut identical = 0;
    for (name, version, stem) in &names {
        let source = corpus.join(version).join(format!("{stem}.ref.musicxml"));
        let reference = corpus
            .join("midi")
            .join(version)
            .join(format!("{stem}.ref.mid"));
        match compare(name, &source, &reference) {
            Ok(()) => {
                identical += 1;
                println!("{name} : identique");
            }
            Err(differences) => {
                println!("{name} : écart");
                for difference in differences {
                    println!("    {difference}");
                }
            }
        }
    }
    println!("MusicXML → MIDI : {identical}/{} identiques", names.len());

    let mut regressions = Vec::new();
    for expected in EXPECTED_IDENTICAL {
        let Some((name, version, stem)) = names
            .iter()
            .find(|(name, _, _)| name == expected)
            .map(|(_, version, stem)| (expected, version, stem))
        else {
            panic!("{expected} est listé dans EXPECTED_IDENTICAL mais absent des références");
        };
        let source = corpus.join(version).join(format!("{stem}.ref.musicxml"));
        let reference = corpus
            .join("midi")
            .join(version)
            .join(format!("{stem}.ref.mid"));
        if let Err(differences) = compare(name, &source, &reference) {
            regressions.push(format!("{name} : {}", differences.join(" ; ")));
        }
    }
    assert!(
        regressions.is_empty(),
        "régressions MusicXML → MIDI :\n{}",
        regressions.join("\n")
    );
}
