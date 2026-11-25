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

//! Capacité réelle du Standard MIDI File, mesurée sur la performance écrite.
//!
//! La règle de `docs/agents/model-modularisation.md` §5.3 est « 100 % = ce que
//! le format porte lui-même ». Un SMF ne porte pas les ticks *écrits* d'une
//! partition : il porte des messages Note On/Off datés (reprises déroulées,
//! durées de tenue, liaisons fusionnées, ornementes décalés) et des
//! méta-événements. L'oracle de ce test est donc **le fichier MIDI produit**,
//! lu avec `midly` indépendamment de l'importeur de `scoretk`. On compare ce
//! que le SMF contient à ce que l'importeur en a fait, champ par champ.
//!
//! Diagnostic uniquement : aucun pourcentage n'est asserté, comme dans
//! `layer_coverage.rs`. Les tests ci-dessous vérifient que l'oracle lit bien
//! les champs qu'il prétend mesurer.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use scoretk::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk::{read_midi_with, write, Format, MidiImportOptions, MidiOptions, Options};

/// Ticks MIDI par noire dans les fichiers écrits par `scoretk-midi-export`.
const PPQ: i64 = 480;
/// Unités du modèle par ronde : `4 * PPQ`.
const UNITS_PER_WHOLE: u32 = PPQ as u32 * 4;

/// Attaque telle que le SMF la porte : `(hauteur, attaque, durée, vélocité,
/// canal)`. Les ticks sont ceux du fichier, pas ceux de la gravure.
type NoteKey = (u8, i64, i64, u8, u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    reference: usize,
    matched: usize,
}

impl Counts {
    fn add(&mut self, reference: usize, matched: usize) {
        self.reference += reference;
        self.matched += matched;
    }

    fn percent(self) -> f64 {
        if self.reference == 0 {
            100.0
        } else {
            self.matched as f64 * 100.0 / self.reference as f64
        }
    }
}

/// Extrait les attaques d'un SMF en appariant Note On / Note Off **par piste
/// et par (canal, hauteur)**, comme un synthétiseur et comme
/// `MidiTrack::mergeNoteOnOff` (`midishared/midifile.cpp`).
fn smf_notes(bytes: &[u8]) -> Vec<NoteKey> {
    let smf = Smf::parse(bytes).expect("un SMF valide");
    let mut result = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        let mut open: Vec<(u8, u8, i64, u8)> = Vec::new(); // canal, hauteur, attaque, vélocité
        for event in track {
            tick += i64::from(event.delta.as_int());
            let TrackEventKind::Midi {
                channel,
                ref message,
            } = event.kind
            else {
                continue;
            };
            let channel = channel.as_int();
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    open.push((channel, key.as_int(), tick, vel.as_int()))
                }

                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    let pitch = key.as_int();
                    if let Some(index) = open
                        .iter()
                        .position(|(c, p, _, _)| *c == channel && *p == pitch)
                    {
                        let (channel, pitch, onset, velocity) = open.remove(index);
                        // `exportmidi.cpp` écrit le Note Off à `fin - 1` : la
                        // fin de la performance est le tick du Note Off + 1.
                        // Un Note On de vélocité nulle sans note ouverte est
                        // un « restrike » de `fixupMIDI`, pas une attaque.
                        if tick > onset {
                            result.push((pitch, onset, tick - onset + 1, velocity, channel));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    result.sort();
    result
}

/// Extrait les attaques du modèle importé. La durée est la durée **jouée**
/// (`ChordRest::ticks`), sommée sur une chaîne de liaisons : le SMF n'a qu'une
/// attaque pour une note liée, même si l'import la découpe en fragments.
fn model_notes(score: &Score) -> Vec<NoteKey> {
    let mut by_id: HashMap<usize, (Fraction, Fraction, u8)> = HashMap::new(); // id → (tick, durée, canal)
    for measure in &score.measures {
        for segment in &measure.segments {
            for (track, element) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = element else {
                    continue;
                };
                let duration = chord_rest
                    .ticks
                    .unwrap_or_else(|| chord_rest.duration.ticks());
                let staff = track.staff().raw() as usize;
                let channel = score
                    .staves
                    .get(staff)
                    .and_then(|item| score.parts.get(item.part.index()))
                    .and_then(|part| part.instrument.midi_channel)
                    .unwrap_or(0);
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for id in &chord.notes {
                    by_id.insert(id.index(), (segment.tick, duration, channel));
                }
            }
        }
    }
    let mut result = Vec::new();
    for (id, note) in score.notes.iter().enumerate() {
        if note.tie_back.is_some() || !by_id.contains_key(&id) {
            continue;
        }
        let (tick, _, channel) = by_id[&id];
        let mut duration = by_id[&id].1;
        let mut next = note.tie_forward;
        while let Some(following) = next {
            let Some((_, length, _)) = by_id.get(&following.index()) else {
                break;
            };
            duration += *length;
            next = score.notes[following.index()].tie_forward;
        }
        result.push((
            note.pitch,
            tick.ticks(UNITS_PER_WHOLE),
            duration.ticks(UNITS_PER_WHOLE).max(1),
            note.playback.velocity.unwrap_or(0),
            channel,
        ));
    }
    result.sort();
    result
}

#[derive(Clone, Default)]
struct FieldCounts {
    combined: Counts,
    pitch: Counts,
    onset: Counts,
    duration: Counts,
    velocity: Counts,
    channel: Counts,
}

/// Compare deux listes d'attaques comme des multi-ensembles. Chaque champ est
/// aussi compté séparément : un canal erroné ne masque pas une bonne hauteur.
fn count_notes(expected: &[NoteKey], actual: &[NoteKey]) -> FieldCounts {
    fn multiset(values: impl Iterator<Item = u64>) -> BTreeMap<u64, usize> {
        let mut result = BTreeMap::new();
        for value in values {
            *result.entry(value).or_insert(0) += 1;
        }
        result
    }
    fn matches(expected: BTreeMap<u64, usize>, actual: &mut BTreeMap<u64, usize>) -> usize {
        expected
            .into_iter()
            .map(|(key, wanted)| {
                let found = actual.get_mut(&key).map_or(0, |count| {
                    let taken = (*count).min(wanted);
                    *count -= taken;
                    taken
                });
                found
            })
            .sum()
    }

    // Un SMF ne dit pas si le Note Off est à la fin incluse ou exclue :
    // `exportmidi.cpp` écrit `start + duration - 1` pour les notes écrites,
    // mais pas pour les accords réalisés. La durée jouée est donc comparée à
    // un tick près, les autres champs à l'identique.
    fn close_durations(expected: &mut [i64], actual: &mut [i64]) -> usize {
        expected.sort_unstable();
        actual.sort_unstable();
        let mut matched = 0;
        let mut index = 0;
        for duration in expected.iter() {
            while index < actual.len() && actual[index] < *duration - 1 {
                index += 1;
            }
            if index < actual.len() && (actual[index] - *duration).abs() <= 1 {
                matched += 1;
                index += 1;
            }
        }
        matched
    }

    let mut result = FieldCounts::default();
    let mut expected_groups: BTreeMap<(u8, i64, u8, u8), Vec<i64>> = BTreeMap::new();
    let mut actual_groups: BTreeMap<(u8, i64, u8, u8), Vec<i64>> = BTreeMap::new();
    for (pitch, onset, duration, velocity, channel) in expected {
        expected_groups
            .entry((*pitch, *onset, *velocity, *channel))
            .or_default()
            .push(*duration);
    }
    for (pitch, onset, duration, velocity, channel) in actual {
        actual_groups
            .entry((*pitch, *onset, *velocity, *channel))
            .or_default()
            .push(*duration);
    }
    let mut combined = 0;
    for (key, mut durations) in expected_groups {
        let mut others = actual_groups.remove(&key).unwrap_or_default();
        combined += close_durations(&mut durations, &mut others);
    }
    result.combined.add(expected.len(), combined);

    let by_pitch: fn(&NoteKey) -> u64 = |n| u64::from(n.0);
    let by_onset: fn(&NoteKey) -> u64 = |n| n.1.max(0) as u64;
    let by_velocity: fn(&NoteKey) -> u64 = |n| u64::from(n.3);
    let by_channel: fn(&NoteKey) -> u64 = |n| u64::from(n.4);
    for (field, counts) in [
        (by_pitch, &mut result.pitch),
        (by_onset, &mut result.onset),
        (by_velocity, &mut result.velocity),
        (by_channel, &mut result.channel),
    ] {
        counts.add(
            expected.len(),
            matches(
                multiset(expected.iter().map(field)),
                &mut multiset(actual.iter().map(field)),
            ),
        );
    }
    let mut expected_durations: Vec<i64> = expected.iter().map(|note| note.2).collect();
    let mut actual_durations: Vec<i64> = actual.iter().map(|note| note.2).collect();
    result.duration.add(
        expected.len(),
        close_durations(&mut expected_durations, &mut actual_durations),
    );
    result
}

fn corpus_scores() -> Vec<(String, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    let mut scores = Vec::new();
    for version in ["v3", "v4"] {
        let mut paths: Vec<PathBuf> = fs::read_dir(root.join(version))
            .expect("corpus directory")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_stem()
                .expect("score file name")
                .to_string_lossy()
                .into_owned();
            scores.push((format!("{version}/{name}"), path));
        }
    }
    scores
}

fn reference_model(path: &Path) -> Score {
    let bytes = fs::read(path).expect("read .mscx reference");
    scoretk::read(&bytes, Format::Mscx).expect("MuseScore import of the reference")
}

/// Round trip `partition → SMF → modèle`, mesuré contre le contenu du SMF.
///
/// Les cinq champs que le format porte pour une note sont la hauteur,
/// l'attaque, la durée jouée (jusqu'au Note Off), la vélocité du Note On et le
/// canal. L'option `smf_exact_ticks` est le chemin de capacité : le défaut
/// 4.7.4 requantifie volontairement vers la grille de notation.
#[test]
fn midi_note_capacity_table() {
    let scores = corpus_scores();
    let exact = MidiImportOptions {
        smf_exact_ticks: true,
        ..Default::default()
    };
    let exact_text = MidiImportOptions {
        smf_exact_ticks: true,
        smf_text: true,
        ..Default::default()
    };
    let text_write = Options {
        midi: MidiOptions {
            smf_text: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let default_write = Options::default();
    let cases: Vec<(&str, &Options, Option<&MidiImportOptions>)> = vec![
        ("default (4.7.4)", &default_write, None),
        ("smf_exact_ticks", &default_write, Some(&exact)),
        ("exact + smf_text", &text_write, Some(&exact_text)),
    ];

    println!(
        "\n=== MIDI note capacity: model re-read vs. the written SMF ({} scores) ===",
        scores.len()
    );
    println!(
        "{:<22} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "reader / writer", "combined", "pitch", "onset", "duration", "velocity", "channel"
    );
    let mut all_totals: Vec<(&str, FieldCounts)> = Vec::new();
    for (label, write_options, read_options) in cases {
        let mut totals = FieldCounts::default();
        let mut per_file: Vec<(String, Counts)> = Vec::new();
        for (name, path) in &scores {
            let score = reference_model(path);
            let Ok(bytes) = write(&score, Format::Midi, write_options) else {
                continue;
            };
            let expected = smf_notes(&bytes);
            let imported = match read_options {
                Some(options) => read_midi_with(&bytes, options),
                None => scoretk::read(&bytes, Format::Midi),
            };
            let Ok(imported) = imported else {
                continue;
            };
            let actual = model_notes(&imported);
            let counts = count_notes(&expected, &actual);
            totals
                .combined
                .add(counts.combined.reference, counts.combined.matched);
            totals
                .pitch
                .add(counts.pitch.reference, counts.pitch.matched);
            totals
                .onset
                .add(counts.onset.reference, counts.onset.matched);
            totals
                .duration
                .add(counts.duration.reference, counts.duration.matched);
            totals
                .velocity
                .add(counts.velocity.reference, counts.velocity.matched);
            totals
                .channel
                .add(counts.channel.reference, counts.channel.matched);
            per_file.push((name.clone(), counts.combined));
        }
        println!(
            "{:<22} {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}%",
            label,
            totals.combined.percent(),
            totals.pitch.percent(),
            totals.onset.percent(),
            totals.duration.percent(),
            totals.velocity.percent(),
            totals.channel.percent(),
        );
        per_file.sort_by(|left, right| {
            let left_rate = left.1.percent();
            let right_rate = right.1.percent();
            left_rate
                .partial_cmp(&right_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let worst: Vec<String> = per_file
            .iter()
            .take(8)
            .map(|(name, count)| format!("{name} {}/{}", count.matched, count.reference))
            .collect();
        println!("    worst files: {}", worst.join(", "));
        all_totals.push((label, totals));
    }
    println!();

    let row = |label: &str| {
        all_totals
            .iter()
            .find(|(name, _)| *name == label)
            .expect("a measured row")
            .1
            .clone()
    };
    let default = row("default (4.7.4)");
    let exact = row("smf_exact_ticks");
    // Le défaut 4.7.4 requantifie les durées : il ne mesure pas la capacité
    // du format. En mode exact, hauteur, attaque et vélocité sont portées à
    // 100 % et la durée l'est à un tick près (fin inclusive/exclusive).
    assert_eq!(exact.pitch.percent(), 100.0);
    assert_eq!(exact.onset.percent(), 100.0);
    assert_eq!(exact.velocity.percent(), 100.0);
    assert!(exact.duration.percent() >= 99.0);
    assert!(exact.combined.percent() >= 98.0);
    assert!(default.duration.percent() < 50.0);
}

#[test]
fn smf_oracle_reads_the_performance_it_is_given() {
    // Reduced oracle: one note at tick 480, gate 0.95 of a quarter (912 ticks),
    // velocity 80 on channel 2, plus a note-off written as Note On vel 0.
    let events: Vec<midly::TrackEvent<'_>> = vec![
        midly::TrackEvent {
            delta: 0.into(),
            kind: TrackEventKind::Midi {
                channel: 2.into(),
                message: MidiMessage::NoteOn {
                    key: 60.into(),
                    vel: 80.into(),
                },
            },
        },
        midly::TrackEvent {
            delta: 912.into(),
            kind: TrackEventKind::Midi {
                channel: 2.into(),
                message: MidiMessage::NoteOn {
                    key: 60.into(),
                    vel: 0.into(),
                },
            },
        },
        midly::TrackEvent {
            delta: 1.into(),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let mut smf = Smf::new(midly::Header::new(
        midly::Format::Parallel,
        midly::Timing::Metrical((PPQ as u16).into()),
    ));
    smf.tracks = vec![events];
    let mut bytes = Vec::new();
    smf.write(&mut bytes).expect("write SMF");
    assert_eq!(smf_notes(&bytes), vec![(60, 0, 913, 80, 2)]);
}
