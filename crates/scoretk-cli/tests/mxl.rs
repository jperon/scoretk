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

//! Entrée MusicXML compressée (`.mxl`) par la CLI.
//!
//! La CLI active `musicxml-import` : `.mxl` → `.mscx` doit fonctionner, et
//! une archive invalide doit échouer explicitement (jamais de sortie vide).

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scoretk"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v4")
        .join(name)
}

#[test]
fn mxl_vers_mscx() {
    let dir = Path::new("/tmp/scoretk-cli-mxl");
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).expect("répertoire");
    let mxl = dir.join("partition.mxl");

    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .args(["--to", "mxl", "--encoding-date", "2026-11-17"])
        .arg("-o")
        .arg(&mxl)
        .output()
        .expect("écriture .mxl");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Détection par contenu : l'extension `.mxl` suffit, mais `--from` absent
    // vérifie aussi que l'archive est reconnue comme MusicXML compressé et
    // non comme `.mscz`.
    let output = binary()
        .arg(&mxl)
        .arg("-o")
        .arg(dir.join("sortie.mscx"))
        .output()
        .expect("lecture .mxl");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let texte = std::fs::read_to_string(dir.join("sortie.mscx")).expect("sortie .mscx");
    assert!(texte.contains("<museScore"), "{texte}");
    assert!(
        texte.contains("<Chord>") || texte.contains("<Rest>"),
        "{texte}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn list_formats_mxl_en_lecture() {
    let output = binary().arg("--list-formats").output().expect("exécution");
    assert!(output.status.success());
    let texte = String::from_utf8(output.stdout).expect("UTF-8");
    let ligne = texte
        .lines()
        .find(|ligne| ligne.starts_with("mxl"))
        .expect("ligne mxl");
    assert_eq!(ligne.matches("oui").count(), 2, "{ligne}");
}
