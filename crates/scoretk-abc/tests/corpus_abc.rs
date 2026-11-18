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

//! Instantanés de l'écriture ABC sur tout le corpus v3/v4.
//!
//! Chaque `.mscx` est converti et comparé à la sortie versionnée dans
//! `tests/corpus/abc/`. Pour accepter un changement de forme volontaire,
//! régénérer les instantanés :
//!
//! ```sh
//! UPDATE_ABC_SNAPSHOTS=1 cargo test -p scoretk-abc --test corpus_abc
//! ```

use std::fs;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

fn convertir(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    let container = scoretk_musescore_import::MsczContainer::from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("conteneur de {path:?} : {error}"));
    let score = scoretk_musescore_import::read::read_score(&container)
        .unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    scoretk_abc::write_abc(&score).unwrap_or_else(|error| panic!("écriture de {path:?} : {error}"))
}

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
    assert!(!fichiers.is_empty(), "corpus vide");
    fichiers
}

fn instantane(path: &Path) -> PathBuf {
    let version = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .expect("dossier de version");
    let tige = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("nom de fichier");
    corpus().join("abc").join(format!("{version}_{tige}.abc"))
}

#[test]
fn conversions_egales_aux_instantanes() {
    let mise_a_jour = std::env::var_os("UPDATE_ABC_SNAPSHOTS").is_some();
    let mut mises_a_jour = 0usize;
    let mut compares = 0usize;

    for path in fichiers() {
        let produit = convertir(&path);
        let reference = instantane(&path);
        if mise_a_jour {
            fs::create_dir_all(reference.parent().expect("dossier"))
                .expect("création du dossier d'instantanés");
            fs::write(&reference, &produit)
                .unwrap_or_else(|error| panic!("écriture de {reference:?} : {error}"));
            mises_a_jour += 1;
            continue;
        }
        let attendu = fs::read_to_string(&reference).unwrap_or_else(|error| {
            panic!(
                "instantané {reference:?} illisible ({error}) : \
                 lancer UPDATE_ABC_SNAPSHOTS=1 cargo test -p scoretk-abc --test corpus_abc"
            )
        });
        assert_eq!(produit, attendu, "instantané différent pour {path:?}");
        compares += 1;
    }

    if mise_a_jour {
        eprintln!("{mises_a_jour} instantanés réécrits");
    } else {
        eprintln!("{compares} conversions comparées aux instantanés");
    }
}

#[test]
fn sortie_structuree() {
    for path in fichiers() {
        let produit = convertir(&path);
        assert!(
            produit.starts_with("%abc-2.1\nX:1\n"),
            "{path:?} : en-tête ABC absent"
        );
        assert!(produit.contains("\nK:"), "{path:?} : champ K: absent");
        assert!(produit.contains("\nL:1/8\n"), "{path:?} : unité L: absente");
        assert!(produit.ends_with('\n'), "{path:?} : fin de document");
        assert!(
            !produit.lines().any(|line| line.len() > 1000),
            "{path:?} : ligne démesurée"
        );
    }
}

#[test]
fn relecture_de_chaque_instantane() {
    for path in fichiers() {
        let produit = convertir(&path);
        scoretk_abc::read_abc(&produit)
            .unwrap_or_else(|error| panic!("relecture de {path:?} : {error}"));
    }
}
