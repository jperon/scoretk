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

//! Compilation de la sortie du corpus par LilyPond lui-même.
//!
//! Le test ne fait rien quand `lilypond` n'est pas dans le `PATH` (cas de
//! `nix flake check`, qui travaille hors réseau) ; il vérifie sinon que chaque
//! conversion compile sans erreur. Les avertissements de mise en page
//! (débordement de page) sont tolérés : seule une erreur de syntaxe ou de
//! durée fait échouer le test.
//!
//! ```sh
//! nix shell nixpkgs#lilypond --command cargo test -p scoretk-lilypond-export --test lilypond_compile -- --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Avertissements qui signalent un défaut musical ou syntaxique de la sortie.
const AVERTISSEMENTS_INTERDITS: &[&str] = &[
    "échec du contrôle de mesure",
    "trait d'union sans suite",
    "la propriété « stanza » n'existe pas",
];

/// Avertissements de mise en page tolérés : le corpus ne vise pas un rendu
/// de page identique à MuseScore.
const AVERTISSEMENTS_TOLERES: &[&str] = &[
    "pas de configuration initiale viable repérée",
    "a été comprimée",
    "compression du débordement de page",
];

/// Racine du corpus.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

/// Vrai si LilyPond est disponible.
fn lilypond_disponible() -> bool {
    Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|sortie| sortie.status.success())
}

/// Tous les `.mscx` du corpus, triés.
fn fichiers() -> Vec<PathBuf> {
    let mut fichiers = Vec::new();
    for version in ["v3", "v4"] {
        let dossier = corpus().join(version);
        let mut entrees: Vec<PathBuf> = fs::read_dir(&dossier)
            .unwrap_or_else(|error| panic!("lecture de {dossier:?} : {error}"))
            .filter_map(|entree| entree.ok())
            .map(|entree| entree.path())
            .filter(|chemin| {
                chemin
                    .extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        entrees.sort();
        fichiers.extend(entrees);
    }
    fichiers
}

#[test]
fn le_corpus_compile_sans_erreur() {
    if !lilypond_disponible() {
        eprintln!("lilypond absent du PATH : test de compilation ignoré");
        return;
    }
    let dossier = std::env::temp_dir().join(format!("scoretk-lilypond-{}", std::process::id()));
    fs::create_dir_all(&dossier).expect("répertoire temporaire");
    let mut compiles = 0usize;

    for path in fichiers() {
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("lecture {path:?} : {error}"));
        let container = scoretk_musescore_import::MsczContainer::from_bytes(&bytes)
            .unwrap_or_else(|error| panic!("conteneur {path:?} : {error}"));
        let score = scoretk_musescore_import::read::read_score(&container)
            .unwrap_or_else(|error| panic!("lecture {path:?} : {error}"));
        let produit = scoretk_lilypond_export::write_lilypond(&score)
            .unwrap_or_else(|error| panic!("écriture {path:?} : {error}"));
        let tige = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier");
        let source = dossier.join(format!("{tige}.ly"));
        fs::write(&source, produit).expect("écriture de la source");
        let sortie = Command::new("lilypond")
            .current_dir(&dossier)
            .args(["-dno-point-and-click", "--pdf", "-o"])
            .arg(&dossier)
            .arg(&source)
            .output()
            .expect("exécution de lilypond");
        let journal = format!(
            "{}{}",
            String::from_utf8_lossy(&sortie.stdout),
            String::from_utf8_lossy(&sortie.stderr)
        );
        assert!(
            sortie.status.success(),
            "LilyPond échoue sur {path:?} :\n{journal}"
        );
        assert!(
            !journal.contains("erreur fatale") && !journal.contains("Erreur :"),
            "LilyPond signale une erreur sur {path:?} :\n{journal}"
        );
        for motif in AVERTISSEMENTS_INTERDITS {
            assert!(
                !journal.contains(motif),
                "LilyPond signale « {motif} » sur {path:?} :\n{journal}"
            );
        }
        // Seuls les avertissements de mise en page du corpus sont tolérés ;
        // tout autre avertissement est considéré comme bruyant.
        for line in journal.lines() {
            if !line.contains("Avertissement") {
                continue;
            }
            assert!(
                AVERTISSEMENTS_TOLERES
                    .iter()
                    .any(|motif| line.contains(motif)),
                "avertissement LilyPond non toléré sur {path:?} : {line}"
            );
        }
        compiles += 1;
    }

    eprintln!("{compiles} fichiers du corpus compilés par LilyPond");
    let _ = fs::remove_dir_all(&dossier);
}
