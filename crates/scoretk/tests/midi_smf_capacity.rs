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

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use scoretk::model::Score;
use scoretk::{read_midi_with, write, Format, MidiImportOptions, MidiOptions, Options};
use support::midi_smf::*;

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

/// Couverture des kinds que le SMF porte réellement, valeur par valeur.
///
/// Le dénominateur est ce que le fichier contient (`FF 58`, `FF 59`, `FF 51`,
/// `FF 05`, textes, programme/canal/CC), pas l'inventaire des ticks écrits de
/// la partition. `rest` en est absent : `[SMF §Track Chunk]` ne contient que
/// des évènements, et `[SMF §Events and time]` ne définit que Note On/Off.
#[test]
fn midi_kind_capacity_table() {
    let scores = corpus_scores();
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
    let cases: [(&str, &Options, Option<&MidiImportOptions>, bool); 3] = [
        ("default (4.7.4)", &default_write, None, false),
        ("exact + smf_text", &text_write, Some(&exact_text), false),
        (
            "exact + smf_text (no private)",
            &text_write,
            Some(&exact_text),
            true,
        ),
    ];
    println!(
        "\n=== MIDI kind capacity: distinct values carried by the SMF ({} scores) ===",
        scores.len()
    );
    let mut rows: BTreeMap<&str, BTreeMap<&'static str, Counts>> = BTreeMap::new();
    for (label, write_options, read_options, strip) in cases {
        let mut totals: BTreeMap<&'static str, Counts> = BTreeMap::new();
        let mut losses: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for (_, path) in &scores {
            let score = reference_model(path);
            let Ok(written) = write(&score, Format::Midi, write_options) else {
                continue;
            };
            // L'oracle reste le fichier tel qu'écrit : ce sont les faits que
            // le format devrait porter. Seule l'entrée du lecteur est privée
            // des extensions, comme dans §11.8 de `model-modularisation.md`.
            let expected = smf_kind_values(&written);
            let bytes = if strip {
                strip_private_text(&written)
            } else {
                written
            };
            let imported = match read_options {
                Some(options) => read_midi_with(&bytes, options),
                None => scoretk::read(&bytes, Format::Midi),
            };
            let Ok(imported) = imported else {
                continue;
            };
            let actual = model_kind_values(&imported);
            for (kind, values) in &expected {
                let count = totals.entry(kind).or_default();
                count.reference += values.len();
                let here = actual.get(kind).cloned().unwrap_or_default();
                count.matched += values.intersection(&here).count();
                for missing in values.difference(&here) {
                    losses.entry(kind).or_default().push(missing.clone());
                }
            }
        }
        println!("  {label}");
        if label == "exact + smf_text" {
            // Ce que le format porte est restitué : signatures de mesure,
            // programme, volume, panoramique et paroles à 100 %. Les écarts
            // résiduels (armures, tempo, canaux, textes) sont diagnostiqués
            // ci-dessus et dans `docs/agents/midi.md`.
            for kind in ["time-sig", "program", "volume", "pan", "lyric"] {
                assert_eq!(
                    totals[kind].percent(),
                    100.0,
                    "{kind} must be fully carried by the SMF"
                );
            }
        }
        for (kind, count) in &totals {
            println!(
                "    {:<20} {:>7.2}%  {}/{}",
                kind,
                count.percent(),
                count.matched,
                count.reference
            );
            if count.matched < count.reference {
                let mut missing = losses.get(kind).cloned().unwrap_or_default();
                missing.sort();
                missing.dedup();
                println!(
                    "      missing: {}",
                    missing[..missing.len().min(5)].join(" | ")
                );
            }
        }
        rows.insert(label, totals);
    }
    println!();

    // Les `FF 01` privés ne sont pas des méta standard : sans eux, la couverture
    // des textes et des métadonnées doit baisser, sans toucher aux notes ni
    // aux méta standards (règle des extensions privées, §11.8).
    let with = &rows["exact + smf_text"];
    let without = &rows["exact + smf_text (no private)"];
    for kind in ["staff-text", "title"] {
        assert!(
            without[kind].percent() < with[kind].percent(),
            "{kind}: private prefixes must account for part of the coverage"
        );
    }
    for kind in ["time-sig", "program", "volume", "pan", "lyric"] {
        assert_eq!(with[kind].percent(), without[kind].percent());
    }

    // Aucun kind fantôme : `rest` n'est ni porté par le SMF ni déclaré.
    let kinds = scoretk_midi_import::CAPABILITIES
        .declared_kinds()
        .expect("SMF kinds");
    assert!(!kinds.contains(&"rest"));
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
