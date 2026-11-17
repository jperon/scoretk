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

//! Lecture LilyPond par la CLI (`--lilypond-import` activé par défaut).
//!
//! Les corpus `.ly` sont produits par l'écrivain en mode `\fixed` ; ce test
//! vérifie le chemin `.ly` → `.musicxml` sur une petite source `\relative`
//! écrite à la main.

use std::path::Path;
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scoretk"))
}

const RELATIVE: &str = "\
\\version \"2.24.0\"
\\language \"english\"
\\score { \\new Staff { \\relative c' { c4 d e f g a b c } } }
";

#[test]
fn ly_relative_vers_musicxml() {
    let entree = Path::new("/tmp/scoretk-cli-ly-relative.ly");
    let sortie = Path::new("/tmp/scoretk-cli-ly-relative.musicxml");
    std::fs::write(entree, RELATIVE).expect("entrée");
    let output = binary()
        .arg(entree)
        .args(["--encoding-date", "2026-11-17"])
        .arg("-o")
        .arg(sortie)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let texte = std::fs::read_to_string(sortie).expect("sortie MusicXML");
    assert!(texte.contains("<score-partwise"), "{texte}");
    assert!(texte.contains("<step>C</step>"), "{texte}");
    assert!(texte.contains("<step>B</step>"), "{texte}");
    let _ = std::fs::remove_file(entree);
    let _ = std::fs::remove_file(sortie);
}

#[test]
fn list_formats_ly_en_lecture() {
    let output = binary().arg("--list-formats").output().expect("exécution");
    assert!(output.status.success());
    let texte = String::from_utf8(output.stdout).expect("UTF-8");
    let ligne = texte
        .lines()
        .find(|ligne| ligne.starts_with("ly"))
        .expect("ligne ly");
    assert_eq!(ligne.matches("oui").count(), 2, "{ligne}");
}
