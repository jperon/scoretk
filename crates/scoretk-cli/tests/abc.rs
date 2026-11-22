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

//! CLI `scoretk` et ABC : lecture, écriture, stdin/stdout.

use std::path::PathBuf;
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scoretk"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v4")
        .join(name)
}

/// Chemin de sortie propre à ce binaire de test.
///
/// `CARGO_TARGET_TMPDIR` est créé par Cargo pour les tests d'intégration :
/// plusieurs agents peuvent lancer la suite en parallèle sans se disputer un
/// nom fixe sous `/tmp` (`scoretk-cli-abc.abc`).
fn sortie_temporaire(nom: &str) -> PathBuf {
    let dossier = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("abc");
    std::fs::create_dir_all(&dossier).expect("dossier temporaire");
    dossier.join(nom)
}

const AIR: &str = "\
X:1
T:Air
M:4/4
L:1/8
K:C
C D E F | G4 z4 |
";

#[test]
fn mscx_vers_abc() {
    let sortie = sortie_temporaire("mscx_vers_abc.abc");
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .arg("-o")
        .arg(&sortie)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let texte = std::fs::read_to_string(&sortie).expect("sortie ABC");
    assert!(texte.starts_with("%abc-2.1\nX:1\n"), "{texte}");
    assert!(texte.contains("\nK:"), "{texte}");
    let _ = std::fs::remove_file(&sortie);
}

#[test]
fn abc_vers_musicxml() {
    let entree = sortie_temporaire("abc_vers_musicxml.abc");
    let sortie = sortie_temporaire("abc_vers_musicxml.musicxml");
    std::fs::write(&entree, AIR).expect("entrée");
    let output = binary()
        .arg(&entree)
        .args(["--encoding-date", "2026-11-17"])
        .arg("-o")
        .arg(&sortie)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let texte = std::fs::read_to_string(&sortie).expect("sortie MusicXML");
    assert!(texte.contains("<score-partwise"), "{texte}");
    assert!(texte.contains("<step>G</step>"), "{texte}");
    let _ = std::fs::remove_file(&entree);
    let _ = std::fs::remove_file(&sortie);
}

#[test]
fn abc_stdin_stdout() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = binary()
        .args(["-", "--from", "abc", "--to", "musicxml"])
        .args(["--encoding-date", "2026-11-17"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lancement");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(AIR.as_bytes())
        .expect("écriture stdin");
    let output = child.wait_with_output().expect("attente");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let texte = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(texte.contains("<score-partwise"), "{texte}");
}

#[test]
fn list_formats_abc_actif() {
    let output = binary().arg("--list-formats").output().expect("exécution");
    assert!(output.status.success());
    let texte = String::from_utf8(output.stdout).expect("UTF-8");
    let ligne = texte
        .lines()
        .find(|ligne| ligne.starts_with("abc"))
        .expect("ligne abc");
    assert!(ligne.contains("oui"), "{ligne}");
    assert_eq!(ligne.matches("oui").count(), 2, "{ligne}");
}
