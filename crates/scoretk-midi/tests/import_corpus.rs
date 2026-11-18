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

//! Conformité de la **lecture MIDI** aux références d'import de MuseScore.
//!
//! Pour chaque `tests/corpus/midi/<version>/<nom>.ref.mid`, le test lit le
//! fichier avec [`scoretk_midi::read_midi`], exporte le modèle en MusicXML et
//! compare le noyau musical à
//! `tests/corpus/midi/import/<version>/<nom>.ref.musicxml`, produit par
//! MuseScore 4.7.4 (`scripts/gen-midi-import-references.sh`).
//!
//! Le test affiche `MIDI import : N/M identiques` et n'échoue que sur les
//! fichiers de [`EXPECTED_IDENTICAL`], qui ne doivent pas régresser. Les
//! écarts restants sont documentés dans `docs/agents/midi.md`.

mod core_norm;

use std::fs;
use std::path::{Path, PathBuf};

/// Fichiers dont l'import est identique à MuseScore au niveau du noyau.
const EXPECTED_IDENTICAL: &[&str] = &["mmrest", "repeat68", "repeat69", "testTempoPrecision"];

fn midi_corpus(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("midi")
        .join(version)
}

fn import_reference(version: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("midi")
        .join("import")
        .join(version)
        .join(format!("{name}.ref.musicxml"))
}

/// Compare un fichier et rend `(identique, lignes d'écart, diff)`.
fn compare(version: &str, path: &Path) -> (bool, usize, String) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .trim_end_matches(".ref.mid")
        .to_owned();
    let bytes = fs::read(path).expect("fichier MIDI du corpus");
    let score = scoretk_midi::read_midi(&bytes).expect("lecture MIDI");
    let actual = scoretk_musicxml_export::export_musicxml(&score).expect("export MusicXML");
    let reference_path = import_reference(version, &name);
    let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
        panic!(
            "référence d'import absente : {} ({error})\n\
             lancez scripts/gen-midi-import-references.sh",
            reference_path.display()
        )
    });

    let (changed, diff) = core_norm::diff(
        &core_norm::normalize(&reference),
        &core_norm::normalize(&actual),
    );
    (changed == 0, changed, diff)
}

fn run_corpus(version: &str) {
    let dir = midi_corpus(version);
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("corpus MIDI")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".ref.mid"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "corpus {version} vide");

    let mut identical = 0;
    let mut failures: Vec<String> = Vec::new();
    let mut details = String::new();

    for path in &paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim_end_matches(".ref.mid")
            .to_owned();
        let (same, changed, diff) = compare(version, path);
        if same {
            identical += 1;
            println!("  {version}/{name:<28} identique");
        } else {
            println!("  {version}/{name:<28} {changed} lignes d'écart");
            if std::env::var_os("T42_IMPORT_DIFF").is_some() && details.len() < 60000 {
                details.push_str(&format!("\n--- {version}/{name} ---\n{diff}\n"));
            }
            if EXPECTED_IDENTICAL.contains(&name.as_str()) {
                failures.push(format!("{version}/{name}"));
            }
        }
    }

    println!(
        "MIDI import {version} : {identical}/{} identiques",
        paths.len()
    );
    if std::env::var_os("T42_IMPORT_DIFF").is_some() {
        fs::write("/tmp/t42-import-diff.txt", &details).ok();
    }
    if !failures.is_empty() {
        panic!(
            "régression d'import MIDI sur : {}\n{}",
            failures.join(", "),
            details
        );
    }
}

#[test]
fn import_midi_v4() {
    run_corpus("v4");
}

#[test]
fn import_midi_v3() {
    run_corpus("v3");
}
