// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lot d'exemples MEI 5.1 externes et mesures du
// lecteur indépendantes des références produites par MuseScore.
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

//! Mesures du lecteur MEI sur les encodages officiels MEI 5.1.
//!
//! Les fichiers ne sont pas versionnés : `scripts/fetch-mei-samples.sh` les
//! télécharge hors du dépôt, à un commit épinglé, et vérifie leur empreinte.
//! Le répertoire est donné par `SCORETK_MEI_SAMPLES` (par défaut
//! `$HOME/.cache/scoretk/mei-samples`) ; sans répertoire présent, les tests
//! de mesure impriment un message d'attente et ne prétendent à aucune mesure.

use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("mei-samples")
}

fn cache_dir() -> PathBuf {
    std::env::var_os("SCORETK_MEI_SAMPLES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("HOME for the MEI sample cache"))
                .join(".cache")
                .join("scoretk")
                .join("mei-samples")
        })
        .join("MEI_5.1")
}

/// Une ligne utile du manifeste : chemin, empreinte, taille, catégorie, raison.
fn manifest(name: &str) -> Vec<(String, String, u64, String, String)> {
    let text = fs::read_to_string(corpus_dir().join(name)).expect("MEI sample manifest");
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert!(
                fields.len() == 4 || fields.len() == 5,
                "malformed manifest line: {line}"
            );
            (
                fields[0].to_owned(),
                fields[1].to_owned(),
                fields[2].parse().expect("byte count"),
                fields[3].to_owned(),
                fields.get(4).unwrap_or(&"").to_string(),
            )
        })
        .collect()
}

#[test]
fn mei_sample_manifest_is_pinned_and_cmn_only() {
    let included = manifest("MANIFEST.tsv");
    let excluded = manifest("EXCLUDED.tsv");
    assert_eq!(included.len(), 130);
    assert_eq!(excluded.len(), 37);
    for (path, hash, size, category, _) in &included {
        assert_eq!(category, "CMN");
        assert_eq!(hash.len(), 64, "SHA-256 of {path}");
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(*size > 0);
    }
    // Les 37 autres fichiers sont des fragments d'en-tête, pas des partitions :
    // leur raison d'exclusion est explicite et aucune écriture neume/mensurale
    // n'est étiquetée « CMN ».
    for (_, _, _, _, reason) in &excluded {
        assert!(reason.contains("no <score>"), "{reason}");
    }
    let cache = cache_dir();
    if !cache.is_dir() {
        eprintln!(
            "MEI sample cache absent ({}) : lancez scripts/fetch-mei-samples.sh",
            cache.display()
        );
        return;
    }
    for (path, _, size, _, _) in &included {
        let file = cache.join(path);
        assert!(file.is_file(), "missing sample {}", file.display());
        assert_eq!(file.metadata().expect("sample metadata").len(), *size);
    }
}
