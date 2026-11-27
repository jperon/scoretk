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
    const KINDS: &[&str] = &[
        "note",
        "rest",
        "stem",
        "lyric",
        "tie",
        "slur",
        "time-sig",
        "articulation",
        "tuplet-member",
        "tuplet",
        "dynamic",
        "ornament",
    ];
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    // Fichier → kind → (nombre manquant, exemples).
    let mut by_file: BTreeMap<String, BTreeMap<&'static str, (usize, Vec<String>)>> =
        BTreeMap::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(model) = read(&bytes, Format::Mei) else {
            continue;
        };
        let reference = inventory(&model);
        let Ok(written) = write(&model, Format::Mei, &options) else {
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
    assert_eq!(totals.files, 130);
    assert_eq!(totals.skipped, 0);
    assert!(totals.total.reference > 0);
}

/// Same 130 external encodings, but with the additive MEI 5.1 carriers
/// (`Options::mei_full_support`) enabled. The baseline row above uses the
/// default 4.7.4-faithful writer; this row measures what the format can carry.
#[test]
fn mei_samples_full_support_residual() {
    let Some(paths) = available_lot() else {
        return;
    };
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let mut totals = Totals::default();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(model) = read(&bytes, Format::Mei) else {
            totals.skipped += 1;
            continue;
        };
        let reference = inventory(&model);
        let Ok(written) = write(&model, Format::Mei, &options) else {
            totals.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&written, Format::Mei) else {
            totals.skipped += 1;
            continue;
        };
        totals.add(&reference, &inventory(&candidate));
    }
    totals.print_losses("with mei_full_support (MEI -> model -> MEI -> model)");
    assert_eq!(totals.files, 130);
    assert_eq!(totals.skipped, 0);
}

/// Mesure honnête : les mêmes 130 fichiers, mais le lecteur ne reconnaît
/// aucun porteur privé (les `mscore-*` de MuseScore 4.7.4 compris). Le score
/// mesuré est celui d'un lecteur MEI 5.1 conforme, pas celui d'un lecteur qui
/// décode nos extensions.
#[test]
fn mei_samples_full_support_without_private_carriers() {
    let Some(paths) = available_lot() else {
        return;
    };
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let mut totals = Totals::default();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(model) = read(&bytes, Format::Mei) else {
            totals.skipped += 1;
            continue;
        };
        let reference = inventory(&model);
        let Ok(written) = write(&model, Format::Mei, &options) else {
            totals.skipped += 1;
            continue;
        };
        let sanitized = without_private_carriers(&written);
        let Ok(candidate) = read(&sanitized, Format::Mei) else {
            totals.skipped += 1;
            continue;
        };
        totals.add(&reference, &inventory(&candidate));
    }
    totals.print_losses("mei_full_support, reader without private carriers");
    assert_eq!(totals.files, 130);
    assert_eq!(totals.skipped, 0);
}

/// Un porteur est « privé » dès qu'une valeur d'attribut porte un jeton
/// `mscore-`, `scoretk` ou `gabc` : ni MEI 5.1 ni un encodage CMN externe n'en
/// exige. Les `xml:id` générés (`n12`, `m3s2l1`, …) ne sont pas des porteurs :
/// ce sont des identifiants opaques résolus par `@startid`/`@endid`.
pub(crate) fn private_token(value: &str) -> bool {
    value.split_whitespace().any(|token| {
        token.starts_with("mscore-")
            || token.starts_with("gabc:")
            || token.starts_with("gabc-")
            || token.contains("scoretk")
    }) || value.starts_with("layout:")
}

/// Rejette les porteurs privés d'un document MEI écrit, comme le ferait un
/// lecteur MEI 5.1 qui n'interprète que les attributs normatifs. Les valeurs
/// `mscore-*` de MuseScore 4.7.4 et les nôtres sont toutes retirées.
fn without_private_carriers(bytes: &[u8]) -> Vec<u8> {
    fn clean(node: &scoretk_xml_common::Element) -> scoretk_xml_common::Element {
        scoretk_xml_common::Element {
            name: node.name.clone(),
            attributes: node
                .attributes
                .iter()
                .filter(|(_, value)| !private_token(value))
                .cloned()
                .collect(),
            text: node.text.clone(),
            children: node
                .children
                .iter()
                .filter_map(|child| {
                    if matches!(child.name.as_str(), "annot" | "extMeta") {
                        None
                    } else {
                        Some(clean(child))
                    }
                })
                .collect(),
        }
    }
    fn emit(xml: &mut scoretk_xml_common::XmlWriter, node: &scoretk_xml_common::Element) {
        let attrs: Vec<(&str, &str)> = node
            .attributes
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        if node.children.is_empty() {
            xml.tag_with(&node.name, &attrs, &node.text);
        } else {
            xml.start_element(&node.name, &attrs);
            for child in &node.children {
                emit(xml, child);
            }
            xml.end_element();
        }
    }
    let tree = scoretk_xml_common::parse(bytes).expect("parse written MEI");
    let clean = clean(&tree);
    let mut xml = scoretk_xml_common::XmlWriter::new();
    xml.start_document();
    emit(&mut xml, &clean);
    xml.finish().into_bytes()
}

/// Aucun jeton privé scoretk ne doit apparaître dans une sortie
/// `mei_full_support`. Les `mscore-*` écrits par MuseScore 4.7.4 lui-même
/// (mode par défaut) ne sont pas visés : ce test filtre explicitement la
/// liste des porteurs que seul scoretk invente.
#[test]
fn mei_samples_full_support_has_no_scoretk_private_token() {
    let Some(paths) = available_lot() else {
        return;
    };
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let mut offenders = BTreeMap::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(model) = read(&bytes, Format::Mei) else {
            continue;
        };
        let Ok(written) = write(&model, Format::Mei, &options) else {
            continue;
        };
        let tree = scoretk_xml_common::parse(&written).expect("parse written MEI");
        let mut stack = vec![&tree];
        while let Some(node) = stack.pop() {
            stack.extend(node.children.iter());
            if matches!(node.name.as_str(), "annot" | "extMeta") {
                offenders
                    .entry(format!("<{}>", node.name))
                    .or_insert_with(|| path.display().to_string());
            }
            for (key, value) in &node.attributes {
                let private = value.split_whitespace().any(|token| {
                    token.starts_with("mscore-grace-")
                        || token.starts_with("mscore-ornament-")
                        || token.starts_with("mscore-breath-")
                        || token.starts_with("mscore-tremolo-")
                        || token.starts_with("mscore-chordline-")
                        || token.starts_with("mscore-instrument")
                        || token.starts_with("mscore-tbox")
                        || token.starts_with("mscore-figured-bass")
                        || token.starts_with("mscore-frame-")
                        || token.starts_with("gabc")
                        || token.contains("scoretk")
                });
                if private {
                    offenders
                        .entry(format!("{}@{} = {}", node.name, key, value))
                        .or_insert_with(|| path.display().to_string());
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "scoretk-private carriers: {offenders:?}"
    );
}

/// Audit des porteurs non normatifs dans les 130 sorties `mei_full_support`.
/// Test de mesure : il imprime l'inventaire, il n'impose aucune limite.
#[test]
fn mei_samples_private_carrier_inventory() {
    let Some(paths) = available_lot() else {
        return;
    };
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    // (occurrence, fichiers) par porteur.
    let mut types: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut labels: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut n_values: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut id_shapes: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut elements: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut files_with_private = 0;
    let mut files_with_musescore = 0;
    let mut private_per_file: BTreeMap<String, usize> = BTreeMap::new();
    let mut musescore_per_file: BTreeMap<String, usize> = BTreeMap::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read MEI sample");
        let Ok(model) = read(&bytes, Format::Mei) else {
            continue;
        };
        let Ok(written) = write(&model, Format::Mei, &options) else {
            continue;
        };
        let tree = scoretk_xml_common::parse(&written).expect("parse written MEI");
        let mut stack = vec![&tree];
        let mut seen_types = std::collections::BTreeSet::new();
        let mut seen_labels = std::collections::BTreeSet::new();
        let mut seen_n = std::collections::BTreeSet::new();
        let mut seen_ids = std::collections::BTreeSet::new();
        let mut private = 0;
        let mut musescore = 0;
        while let Some(node) = stack.pop() {
            stack.extend(node.children.iter());
            if matches!(node.name.as_str(), "annot" | "extMeta") {
                let key = format!("<{}>", node.name);
                let entry = elements.entry(key).or_default();
                entry.0 += 1;
                private += 1;
            }
            if node.name == "f" && node.text.contains(';') && node.text.contains('/') {
                elements.entry("<f encoded>".to_owned()).or_default().0 += 1;
            }
            for (key, value) in &node.attributes {
                match key.as_str() {
                    "type" => {
                        seen_types.insert(value.clone());
                        let entry = types.entry(value.clone()).or_default();
                        entry.0 += 1;
                        if value.starts_with("mscore-staff-text")
                            || value.starts_with("mscore-system-text")
                            || value.starts_with("mscore-ending-")
                            || value.starts_with("mscore-manual")
                            || value.starts_with("mscore-beam-")
                            || value.starts_with("mscore-marker-")
                            || value.starts_with("mscore-jump-")
                            || value.starts_with("mscore-repeat-")
                            || value.starts_with("mscore-infer-from-text")
                        {
                            musescore += 1;
                        } else if value.starts_with("mscore-grace-")
                            || value.starts_with("mscore-ornament-")
                            || value.starts_with("mscore-breath-")
                            || value.starts_with("mscore-tremolo-")
                            || value.starts_with("mscore-chordline-")
                            || value.starts_with("mscore-instrument")
                            || value.starts_with("mscore-tbox-")
                            || value.starts_with("mscore-figured-bass")
                            || value.starts_with("mscore-frame-")
                        {
                            private += 1;
                        }
                    }
                    "label" => {
                        seen_labels.insert(value.clone());
                        let entry = labels.entry(value.clone()).or_default();
                        entry.0 += 1;
                        if value.starts_with("mscore-") {
                            private += 1;
                        }
                    }
                    "n" => {
                        seen_n.insert(value.clone());
                        n_values.entry(value.clone()).or_default().0 += 1;
                    }
                    "class" => {
                        elements.entry(format!("@class={value}")).or_default().0 += 1;
                    }
                    "id" => {
                        let shape: String = value
                            .chars()
                            .map(|c| if c.is_ascii_digit() { '9' } else { c })
                            .collect();
                        seen_ids.insert(shape.clone());
                        id_shapes.entry(shape).or_default().0 += 1;
                    }
                    _ => {}
                }
            }
        }
        // Un porteur ne compte qu'une fois par fichier dans la colonne « fichiers ».
        for (map, seen) in [
            (&mut types, &seen_types),
            (&mut labels, &seen_labels),
            (&mut n_values, &seen_n),
            (&mut id_shapes, &seen_ids),
        ] {
            for key in seen {
                map.get_mut(key).expect("counted carrier").1 += 1;
            }
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if private > 0 {
            files_with_private += 1;
            *private_per_file.entry(name.clone()).or_default() += private;
        }
        if musescore > 0 {
            files_with_musescore += 1;
            *musescore_per_file.entry(name).or_default() += musescore;
        }
    }
    let report = |label: &str, map: &BTreeMap<String, (usize, usize)>| {
        println!("\n=== {label} (occurrence, files) ===");
        let mut rows: Vec<_> = map.iter().collect();
        rows.sort_by(|left, right| right.1 .0.cmp(&left.1 .0).then_with(|| left.0.cmp(right.0)));
        for (value, (count, files)) in rows.into_iter().take(80) {
            println!("  {count:>6} {files:>4}  {value}");
        }
    };
    report("@type", &types);
    report("@label", &labels);
    report("@n", &n_values);
    report("xml:id shapes", &id_shapes);
    println!("\n=== elements (occurrence) ===");
    for (name, (count, _)) in &elements {
        println!("  {count:>6}  {name}");
    }
    println!("\n=== scoretk-private occurrences: {files_with_private} files ===");
    let mut ranked: Vec<_> = private_per_file.iter().collect();
    ranked.sort_by_key(|entry| std::cmp::Reverse(*entry.1));
    for (name, count) in ranked.into_iter().take(20) {
        println!("  {count:>6}  {name}");
    }
    println!("\n=== MuseScore 4.7.4 own tokens: {files_with_musescore} files ===");
    let mut ranked: Vec<_> = musescore_per_file.iter().collect();
    ranked.sort_by_key(|entry| std::cmp::Reverse(*entry.1));
    for (name, count) in ranked.into_iter().take(20) {
        println!("  {count:>6}  {name}");
    }
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
    assert!(unreadable.is_empty(), "unreadable samples: {unreadable:?}");
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
    // Les 130 partitions du lot sont lisibles ; seuls les fragments sans
    // `<score>` (déjà documentés) peuvent renvoyer `Error::NoScore`.
    assert!(failed.len() <= excluded.len());
    for path in failed {
        assert!(
            excluded.contains(&path),
            "a lot file failed to read: {}",
            path.display()
        );
    }
}
