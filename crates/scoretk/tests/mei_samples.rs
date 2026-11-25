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
//!
//! Deux mesures complémentaires :
//!
//! - `MEI → modèle → MEI → modèle`, comparé comme dans `layer_coverage`,
//!   c'est-à-dire que la référence est le modèle issu du fichier externe ;
//! - un **audit d'import** : pour chaque élément/attribut MEI présent dans les
//!   sources, le lecteur le mappe-t-il vers le modèle ?

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::{compare_with_kinds, inventory, LayerCounts, LayerInventory};
use scoretk::{read, write, Format, Options};
use scoretk_mei_export::CAPABILITIES as MEI_EXPORT;
use scoretk_mei_import::{audit_mei, CAPABILITIES as MEI_IMPORT};

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

fn lot() -> Vec<PathBuf> {
    manifest("MANIFEST.tsv")
        .into_iter()
        .map(|(path, _, _, _, _)| cache_dir().join(path))
        .collect()
}

fn available_lot() -> Option<Vec<PathBuf>> {
    let paths = lot();
    if !cache_dir().is_dir() {
        eprintln!(
            "MEI sample cache absent ({}) : lancez scripts/fetch-mei-samples.sh ; aucune mesure effectuée",
            cache_dir().display()
        );
        return None;
    }
    for path in &paths {
        assert!(path.is_file(), "missing sample {}", path.display());
    }
    Some(paths)
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
    if let Some(paths) = available_lot() {
        for (path, _, size, _, _) in &included {
            assert_eq!(
                cache_dir()
                    .join(path)
                    .metadata()
                    .expect("sample metadata")
                    .len(),
                *size
            );
        }
        assert_eq!(paths.len(), 130);
    }
}

/// Comptes agrégés d'une mesure layer-coverage, avec les pertes par kind.
#[derive(Default)]
struct Totals {
    files: usize,
    skipped: usize,
    total: LayerCounts,
    missing_kinds: BTreeMap<&'static str, usize>,
}

impl Totals {
    fn add(&mut self, reference: &LayerInventory, candidate: &LayerInventory) {
        self.files += 1;
        let declared = MEI_IMPORT.declared().union(MEI_EXPORT.declared());
        let kinds = MEI_IMPORT.declared_kinds().or(MEI_EXPORT.declared_kinds());
        let coverage = compare_with_kinds(reference, candidate, declared, kinds);
        self.total.reference += coverage.total.reference;
        self.total.matched += coverage.total.matched;
        self.total.extra += coverage.total.extra;
        for ((_, kind), count) in coverage.missing_kinds {
            *self.missing_kinds.entry(kind).or_insert(0) += count;
        }
    }

    fn percent(&self) -> f64 {
        self.total.percent()
    }

    fn print_losses(&self, label: &str) {
        println!(
            "\n=== MEI samples, per-kind losses: {label} ({:.2} %, {} files, {} skipped) ===",
            self.percent(),
            self.files,
            self.skipped
        );
        let mut gaps: Vec<_> = self
            .missing_kinds
            .iter()
            .map(|(kind, count)| (*kind, *count))
            .collect();
        gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        for (kind, count) in gaps {
            println!("    {:<28} {:>6}", kind, count);
        }
    }
}

fn round_trip_totals(paths: &[PathBuf]) -> Totals {
    let mut totals = Totals::default();
    for path in paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(reference) = read(&bytes, Format::Mei) else {
            totals.skipped += 1;
            continue;
        };
        let reference = inventory(&reference);
        let Ok(written) = write(
            &read(&bytes, Format::Mei).expect("MEI read for round trip"),
            Format::Mei,
            &Options::default(),
        ) else {
            totals.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&written, Format::Mei) else {
            totals.skipped += 1;
            continue;
        };
        let candidate = inventory(&candidate);
        totals.add(&reference, &candidate);
    }
    totals
}

/// Diagnostic ciblé : pour les kinds dominants, quels fichiers et quels
/// `(tick, staff, value)` manquent ? Test de mesure, aucune assertion.
#[test]
fn mei_samples_core_loss_diagnostic() {
    let Some(paths) = available_lot() else {
        return;
    };
    const KINDS: &[&str] = &["note", "rest", "stem", "lyric", "tie", "slur", "time-sig"];
    // Fichier → kind → (nombre manquant, exemples).
    let mut by_file: BTreeMap<String, BTreeMap<&'static str, (usize, Vec<String>)>> =
        BTreeMap::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(model) = read(&bytes, Format::Mei) else {
            continue;
        };
        let reference = inventory(&model);
        let Ok(written) = write(&model, Format::Mei, &Options::default()) else {
            continue;
        };
        let Ok(candidate) = read(&written, Format::Mei) else {
            continue;
        };
        let candidate = inventory(&candidate);
        let mut expected: BTreeMap<(&'static str, String, String, String), usize> = BTreeMap::new();
        let mut actual: BTreeMap<(&'static str, String, String, String), usize> = BTreeMap::new();
        for (bucket, source) in [(&mut expected, &reference), (&mut actual, &candidate)] {
            for item in &source.items {
                if KINDS.contains(&item.kind) {
                    let key = (
                        item.kind,
                        item.tick.map(|tick| tick.to_string()).unwrap_or_default(),
                        item.staff.map(|s| s.to_string()).unwrap_or_default(),
                        item.value.clone(),
                    );
                    *bucket.entry(key).or_insert(0) += 1;
                }
            }
        }
        let name = path
            .file_name()
            .expect("sample name")
            .to_string_lossy()
            .into_owned();
        for &kind in KINDS {
            let mut missing = 0;
            let mut examples = Vec::new();
            for (key, wanted) in &expected {
                if key.0 != kind {
                    continue;
                }
                let found = actual.get_mut(key).map_or(0, |count| {
                    let taken = (*count).min(*wanted);
                    *count -= taken;
                    taken
                });
                for _ in found..*wanted {
                    missing += 1;
                    if examples.len() < 3 {
                        examples.push(format!("tick {} staff {} = {}", key.1, key.2, key.3));
                    }
                }
            }
            if missing > 0 {
                by_file
                    .entry(name.clone())
                    .or_default()
                    .insert(kind, (missing, examples));
            }
        }
    }
    for &kind in KINDS {
        let mut ranked: Vec<_> = by_file
            .iter()
            .filter_map(|(name, kinds)| kinds.get(kind).map(|entry| (name, entry)))
            .collect();
        ranked.sort_by_key(|entry| std::cmp::Reverse(entry.1 .0));
        println!("\n=== {kind}: largest losses by file ===");
        for (name, (count, examples)) in ranked.into_iter().take(8) {
            println!("  {name}: {count} missing; {}", examples.join(" | "));
        }
    }
}

#[test]
fn mei_samples_round_trip_table() {
    let Some(paths) = available_lot() else {
        return;
    };
    let totals = round_trip_totals(&paths);
    totals.print_losses("MEI -> model -> MEI -> model");
    assert_eq!(totals.files + totals.skipped, 130);
    // Avant le correctif « docStarts » : deux documents valides dont la racine
    // est `meiHead` / `music` sont refusés par `read_mei` (racine `mei` exigée).
    assert!(totals.skipped <= 2, "{} skipped", totals.skipped);
    assert!(totals.total.reference > 0);
}

#[test]
fn mei_samples_import_completeness_table() {
    let Some(paths) = available_lot() else {
        return;
    };
    let mut all = scoretk_mei_import::ImportAudit::default();
    let mut unreadable = Vec::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        match read(&bytes, Format::Mei) {
            Ok(_) => {}
            Err(error) => unreadable.push(format!("{}: {error}", path.display())),
        }
        all.merge(&audit_mei(&bytes).expect("audit MEI sample"));
    }
    println!(
        "\n=== MEI import completeness on {} official samples (untrusted data) ===",
        paths.len()
    );
    println!(
        "elements present {} / ignored {}; attributes present {} / ignored {}",
        all.present_element_total(),
        all.ignored_element_total(),
        all.present_attribute_total(),
        all.ignored_attribute_total()
    );
    println!("  most ignored elements (present count in parentheses):");
    for (name, count) in all.ignored_elements().into_iter().take(30) {
        println!("    {:<28} {:>7}", name, count);
    }
    println!("  most ignored attributes (element@attribute):");
    for (element, attribute, count) in all.ignored_attributes().into_iter().take(30) {
        println!("    {:<20} @{:<24} {:>7}", element, attribute, count);
    }
    println!();
    // Deux documents `mei-all_anyStart` ont une racine `meiHead` / `music` :
    // le lecteur n'accepte encore que `mei` (corrigé dans une étape suivante).
    assert!(unreadable.len() <= 2, "unreadable samples: {unreadable:?}");
    assert!(all.present_element_total() > 100_000);
    assert!(all.ignored_element_total() > 0);
}

#[test]
fn mei_samples_never_panic() {
    let Some(mut paths) = available_lot() else {
        return;
    };
    // Les fragments exclus sont également lus : un document sans `<score>`
    // doit renvoyer une erreur, jamais paniquer.
    for line in manifest("EXCLUDED.tsv") {
        paths.push(cache_dir().join(line.0));
    }
    let excluded: Vec<PathBuf> = manifest("EXCLUDED.tsv")
        .into_iter()
        .map(|row| cache_dir().join(row.0))
        .collect();
    let mut failed = Vec::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        if read(&bytes, Format::Mei).is_err() {
            failed.push(path.clone());
        }
    }
    // Les 130 partitions du lot sont lisibles, sauf les deux documents
    // `docStarts` dont la racine n'est pas `mei` (étape de correction suivante).
    // Les fragments exclus peuvent renvoyer `Error::NoScore` sans paniquer.
    let known_roots = [
        "docStarts/Doc_starts_with_meiHead.mei",
        "docStarts/Doc_starts_with_music.mei",
    ]
    .map(|name| cache_dir().join(name));
    assert!(failed.len() <= excluded.len() + known_roots.len());
    for path in failed {
        assert!(
            excluded.contains(&path) || known_roots.contains(&path),
            "a lot file failed to read: {}",
            path.display()
        );
    }
}
