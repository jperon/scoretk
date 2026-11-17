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

//! Vérification par des outils ABC externes (`abcm2ps`, `abc2midi`).
//!
//! Le test est **ignoré par défaut** : il exige les binaires `abcm2ps` et
//! `abc2midi`, absents du shell de développement. Lancer :
//!
//! ```sh
//! scripts/check-abc-external.sh
//! ```
//!
//! Le script compile ce test puis l'exécute dans un `nix shell` fournissant
//! `abcm2ps` et `abcmidi` (nixpkgs). Il vérifie que chaque instantané du
//! corpus et chaque air de `airs.rs` est accepté par les deux outils.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tool(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// Instantanés qu'`abcm2ps` ou `abc2midi` refusent aujourd'hui.
///
/// Les causes sont propres aux outils, pas à la syntaxe produite : n-olets
/// imbriqués (extension hors norme 2.1), clefs anciennes, reprises sans note.
/// Ils restent couverts par les instantanés et l'aller-retour interne.
const IGNORED: &[&str] = &[
    "v3_layout_elements",
    "v3_testHarmony6",
    "v3_testTuplets8",
    "v3_timesig-05",
    "v4_copypaste_parts",
    "v4_linked-ties",
    "v4_mmrest",
    "v4_repeat68",
    "v4_testExcludeInvisibleElements",
    "v4_voiceswitching",
];

const AIRS: &[(&str, &str)] = &[
    (
        "au-clair-de-la-lune",
        "X:1\nT:Au clair de la lune\nM:4/4\nL:1/4\nK:C\nC C C D | E E D z | C C C D | E E D |]\n",
    ),
    (
        "greensleeves",
        "X:1\nT:Greensleeves\nM:6/8\nL:1/8\nK:Am\nA2A c2d | e2e e2d | c2B A2B | A4 z2 |]\n",
    ),
];

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/abc")
}

fn snapshots() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus())
        .expect("instantanés ABC")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "abc"))
        .collect();
    files.sort();
    files
}

#[test]
#[ignore = "nécessite abcm2ps et abc2midi (scripts/check-abc-external.sh)"]
fn sorties_acceptees_par_abcm2ps_et_abc2midi() {
    let Some(abcm2ps) = tool("abcm2ps") else {
        eprintln!("abcm2ps absent : test ignoré");
        return;
    };
    let Some(abc2midi) = tool("abc2midi") else {
        eprintln!("abc2midi absent : test ignoré");
        return;
    };

    let dir = std::env::temp_dir().join(format!("scoretk-abc-externe-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("répertoire temporaire");
    let mut echecs = Vec::new();

    let mut files = snapshots();
    for (name, source) in AIRS {
        let path = dir.join(format!("{name}.abc"));
        fs::write(&path, source).expect("air");
        files.push(path);
    }

    for path in files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("air");
        if IGNORED.contains(&name) {
            continue;
        }
        let output = dir.join(format!("{name}.ps"));
        let status = Command::new(&abcm2ps)
            .arg("-q")
            .arg("-O")
            .arg(&output)
            .arg(&path)
            .status()
            .expect("abcm2ps");
        if !status.success() {
            echecs.push(format!("abcm2ps refuse {name}"));
            continue;
        }
        let midi = dir.join(format!("{name}.mid"));
        let status = Command::new(&abc2midi)
            .arg(&path)
            .arg("-o")
            .arg(&midi)
            .status()
            .expect("abc2midi");
        if !status.success() {
            echecs.push(format!("abc2midi refuse {name}"));
        }
    }

    let _ = fs::remove_dir_all(&dir);
    assert!(echecs.is_empty(), "{}", echecs.join("\n"));
}
