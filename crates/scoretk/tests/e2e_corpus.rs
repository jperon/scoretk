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

//! Test de bout en bout sur le corpus MuseScore 4.
//!
//! Pour chaque `tests/corpus/v4/X.mscx`, on convertit le fichier avec
//! [`scoretk::convert`] et on compare le MusicXML obtenu à la référence
//! `X.ref.musicxml` exportée par MuseScore 4.7.4.
//!
//! La comparaison porte sur le **noyau musical** : les deux documents sont
//! d'abord normalisés, c'est-à-dire réduits à une suite de lignes canoniques.
//! La normalisation retire :
//!
//! - les métadonnées et la mise en page : `<identification>`, `<defaults>`,
//!   `<credit>` et les attributs `default-x`, `default-y`, `relative-x`,
//!   `relative-y`, `width`, `print-object`, etc. ;
//! - les éléments hors du périmètre du noyau : `<direction>`, `<stem>`,
//!   `<beam>`, `<print>`, `<notations>` sauf `tied`/`tuplet`, `<barline>` sauf
//!   les reprises (`<repeat>`).
//!
//! Le test affiche un tableau « fichier : identique / N lignes d'écart » et
//! n'échoue que pour les fichiers de [`EXPECTED_CONFORMANT`] : les autres
//! écarts sont documentés dans `docs/agents/ecarts-noyau.md`.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use common::musicxml_core::{diff, normalize, normalize_full};
use similar::{ChangeTag, TextDiff};

use scoretk::{convert, ConvertOptions, ExportOptions, OutputFormat};

/// Date d'encodage figée : `<identification>` est normalisé de toute façon,
/// mais la sortie reste ainsi entièrement reproductible.
const ENCODING_DATE: &str = "2026-11-16";

/// Dossier d'un corpus versionné.
fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
}

/// Fichiers dont la conversion est aujourd'hui identique à la référence après
/// normalisation. Toute régression sur cette liste fait échouer le test ; les
/// fichiers absents sont suivis dans `docs/agents/ecarts-noyau.md`.
const EXPECTED_CONFORMANT: &[&str] = &[
    "beamStemDir",
    "changevisibility",
    "copypaste_parts",
    "copypastesymbollist-lyrics",
    "dynamics",
    "dynamics_niente",
    "expression-1",
    "hairpin_dynamic_inside",
    "hairpin_dynamic_jump",
    "hairpin_simple",
    "hairpins_and_repeats",
    "linked-ties",
    "mmrest",
    "part-textlines",
    "partExclusion",
    "repeat65",
    "repeat66",
    "repeat68",
    "repeat69",
    "tenuto_accent",
    "testExcludeInvisibleElements",
    "testLocalTimesig1",
    "testNoteheads2",
    "testTempoPrecision",
    "timesig-09",
    "voiceswitching",
];

/// Fichiers du corpus 3.x dont la conversion est identique à la référence
/// après normalisation (voir `docs/agents/ecarts-noyau.md`).
const EXPECTED_CONFORMANT_V3: &[&str] = &[
    "Beam-CrossM1",
    "barline01",
    "barlinedelete",
    "change",
    "clef-1",
    "copypaste_tuplet_01",
    "grace",
    "keysig_78216",
    "layout_elements",
    "lyricsline01",
    "measure-4",
    "repeat01",
    "repeat06",
    "repeat07",
    "repeat11",
    "repeat20",
    "repeat39",
    "selectionfilter1",
    "testBreaksPage",
    "testChordSymbols",
    "testHairpinDynamics",
    "testHarmony6",
    "testMultiInstrumentPart3",
    "testTboxWords1",
    "testTuplets8",
    "testWedge4",
    "timesig-05",
    "undoResequenceMeasure",
    "voices",
];

/// Tronque un diff pour ne pas noyer la sortie du test.
fn truncate_diff(diff: &str, max_lines: usize) -> String {
    let mut lines = diff.lines();
    let kept: Vec<&str> = lines.by_ref().take(max_lines).collect();
    let mut text = kept.join("\n");
    if lines.next().is_some() {
        text.push_str("\n… (diff tronqué)");
    }
    text
}

#[test]
fn corpus_v4_conforme_aux_references() {
    run_corpus("v4", EXPECTED_CONFORMANT);
}

#[test]
fn corpus_v3_conforme_aux_references() {
    run_corpus("v3", EXPECTED_CONFORMANT_V3);
}

/// Statistiques d'écart pour une balise.
#[derive(Default)]
struct TagStat {
    lines: usize,
    files: BTreeSet<String>,
    examples: Vec<String>,
}

/// Résultat de la comparaison complète d'un corpus.
struct FullReport {
    /// Nombre de lignes d'écart par fichier, dans l'ordre alphabétique.
    rows: Vec<(String, usize)>,
    /// Écarts groupés par nom de balise, avec exemples.
    tags: BTreeMap<String, TagStat>,
    /// Diffs unifiés, pour le diagnostic.
    diffs: String,
}

/// Extrait le nom de balise d'une ligne canonique.
fn line_tag(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('<')?;
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    let end = rest
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_owned())
}

/// Enregistre les balises des lignes modifiées d'un diff.
fn collect_tags(
    expected: &[String],
    actual: &[String],
    file: &str,
    tags: &mut BTreeMap<String, TagStat>,
) {
    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
    let text_diff = TextDiff::from_slices(&expected_refs, &actual_refs);
    for change in text_diff.iter_all_changes() {
        if change.tag() == ChangeTag::Equal {
            continue;
        }
        let Some(tag) = line_tag(change.value()) else {
            continue;
        };
        let stat = tags.entry(tag).or_default();
        stat.lines += 1;
        stat.files.insert(file.to_owned());
        let example = change.value().trim();
        let example = if example.chars().count() > 110 {
            format!("{}…", example.chars().take(110).collect::<String>())
        } else {
            example.to_owned()
        };
        if stat.examples.len() < 3 && !stat.examples.contains(&example) {
            stat.examples.push(example);
        }
    }
}

/// Convertit un corpus et compare sa sortie complète (mise en page retirée).
fn run_full_corpus(version: &str) -> FullReport {
    let directory = corpus_dir(version);
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("corpus introuvable dans {} : {error}", directory.display()))
        .map(|entry| entry.expect("entrée de répertoire").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();

    assert!(
        !files.is_empty(),
        "aucun fichier .mscx dans le corpus {version}"
    );

    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut report = FullReport {
        rows: Vec::new(),
        tags: BTreeMap::new(),
        diffs: String::new(),
    };

    for path in &files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier UTF-8")
            .to_owned();

        let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
        let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
            panic!("référence {} illisible : {error}", reference_path.display())
        });
        let expected = normalize_full(&reference);

        let input = fs::read(path).expect("lecture du .mscx");
        let actual = match convert(&input, &options) {
            Ok(bytes) => normalize_full(&String::from_utf8(bytes).expect("MusicXML UTF-8")),
            Err(error) => {
                report.rows.push((name.clone(), usize::MAX));
                report
                    .diffs
                    .push_str(&format!("\n=== {version}/{name} : ERREUR ===\n{error}\n"));
                continue;
            }
        };

        let (changed, diff) = diff(&expected, &actual);
        report.rows.push((name.clone(), changed));
        if changed > 0 {
            collect_tags(&expected, &actual, &name, &mut report.tags);
            report.diffs.push_str(&format!(
                "\n=== {version}/{name} : {changed} lignes d'écart ===\n"
            ));
            report.diffs.push_str(&truncate_diff(&diff, 60));
            report.diffs.push('\n');
        }
    }

    report
}

#[test]
fn corpus_v4_complet() {
    print_full_report("v4", &run_full_corpus("v4"));
}

#[test]
fn corpus_v3_complet() {
    print_full_report("v3", &run_full_corpus("v3"));
}

/// Imprime le tableau et les écarts groupés d'un corpus.
fn print_full_report(version: &str, report: &FullReport) {
    println!("conformité complète du corpus {version} (mise en page pure retirée) :");
    for (name, changed) in &report.rows {
        if *changed == usize::MAX {
            println!("  {name} : ERREUR");
        } else if *changed == 0 {
            println!("  {name} : identique");
        } else {
            println!("  {name} : {changed} lignes d'écart");
        }
    }
    println!("écarts groupés par balise :");
    print_tag_stats(&report.tags);
    println!("{}", report.diffs);
}

/// Imprime les écarts groupés, triés par nombre de lignes décroissant.
fn print_tag_stats(tags: &BTreeMap<String, TagStat>) {
    let mut stats: Vec<(&String, &TagStat)> = tags.iter().collect();
    stats.sort_by(|(left_name, left), (right_name, right)| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| right.files.len().cmp(&left.files.len()))
            .then_with(|| left_name.cmp(right_name))
    });
    for (tag, stat) in stats {
        let mut examples = stat.examples.join(" | ");
        if examples.is_empty() {
            examples.push_str("(aucun exemple)");
        }
        println!(
            "  {tag} : {} lignes, {} fichiers — {examples}",
            stat.lines,
            stat.files.len()
        );
    }
}

/// Produit le contenu de `docs/agents/ecarts-complets.md`.
///
/// Le test est volontairement non bloquant : il mesure l'écart complet pour
/// suivre la parité, sans exiger qu'elle soit atteinte.
#[test]
fn ecarts_complets_document() {
    let v4 = run_full_corpus("v4");
    let v3 = run_full_corpus("v3");

    println!("---DEBUT DOC---");
    println!("# Écarts complets sur le corpus (parité complète)");
    println!();
    println!(
        "Ce document est produit par le test `ecarts_complets_document` de \
         `crates/scoretk/tests/e2e_corpus.rs`. Il compare chaque `.mscx` à sa \
         référence `X.ref.musicxml` après retrait de la **mise en page pure** \
         uniquement : attributs `default-x`, `default-y`, `relative-x`, \
         `relative-y`, `width`, blocs `<defaults>`, `<print>` (et leurs \
         `<system-layout>`/`<staff-layout>`) et `<encoding-date>`. Tout le \
         reste est comparé : `<stem>`, `<beam>`, `<barline>`, `<direction>`, \
         `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc."
    );
    println!();
    println!("## Tableau par fichier");
    println!();
    for (version, report) in [("v3", &v3), ("v4", &v4)] {
        println!("### Corpus {version}");
        println!();
        println!("| Fichier | Lignes d'écart |");
        println!("| --- | ---: |");
        for (name, changed) in &report.rows {
            if *changed == usize::MAX {
                println!("| {name} | ERREUR |");
            } else if *changed == 0 {
                println!("| {name} | **identique** |");
            } else {
                println!("| {name} | {changed} |");
            }
        }
        println!();
    }

    println!("## Écarts groupés par balise");
    println!();
    println!(
        "Fréquence = nombre de lignes canoniques modifiées sur les deux corpus ; \
              fichiers = nombre de fichiers touchés. Tri par fréquence décroissante."
    );
    println!();
    println!("| Balise | Lignes | Fichiers | Exemples |");
    println!("| --- | ---: | ---: | --- |");
    let mut combined: BTreeMap<String, TagStat> = BTreeMap::new();
    for report in [&v3, &v4] {
        for (tag, stat) in &report.tags {
            let entry = combined.entry(tag.clone()).or_default();
            entry.lines += stat.lines;
            entry.files.extend(stat.files.iter().cloned());
            for example in &stat.examples {
                if entry.examples.len() < 3 && !entry.examples.contains(example) {
                    entry.examples.push(example.clone());
                }
            }
        }
    }
    let mut stats: Vec<(&String, &TagStat)> = combined.iter().collect();
    stats.sort_by(|(left_name, left), (right_name, right)| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| right.files.len().cmp(&left.files.len()))
            .then_with(|| left_name.cmp(right_name))
    });
    for (tag, stat) in stats {
        let examples = stat
            .examples
            .iter()
            .map(|example| format!("`{example}`"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "| `{tag}` | {} | {} | {} |",
            stat.lines,
            stat.files.len(),
            examples
        );
    }
    println!();
    println!("## Écarts qui dépendent de la mise en page calculée");
    println!();
    println!(
        "Certains écarts ne viennent pas d'une donnée du `.mscx` mais d'un objet \
         que MuseScore fabrique pendant la mise en page, absente du modèle. \
         Ils sont irréductibles sans porter le moteur de mise en page :\n\n\
         - **direction de hampe d'une ligature croisée** (`v3/moonlight`, \
           9 notes) : `BeamLayout::layout1` fixe la direction de la ligature à \
           partir de `computeUpForMovedCross` et `isBelowCrossBeam`, donc du \
           groupement des ligatures et de la géométrie des notes, pas du \
           `<Beam><StemDirection>` du fichier ;\n\
         - **barres de courtoisie d'un système coupé par la largeur de page** \
           (`v3/voices`, 2 barres `light-light`) : `MeasureLayout::createEndBarLines` \
           ne pose la barre double que si la mesure est la dernière du système \
           (`isLastMeasureInSystem`) et que la suivante change d'armure. Quand \
           le saut de système figure dans le fichier (`<LayoutBreak>`), il est \
           reproduit (voir `export/systems.rs` et `read/measure_repeat.rs`) ; \
           une coupure automatique dépend de la largeur calculée des mesures, \
           donc de toute la mise en page horizontale."
    );
    println!();
    println!("## Diffs");
    println!();
    println!("```diff");
    print!("{}", v4.diffs);
    print!("{}", v3.diffs);
    println!("```");
    println!("---FIN DOC---");
}

/// Convertit tout un corpus et compare chaque fichier à sa référence.
fn run_corpus(version: &str, expected_conformant: &[&str]) {
    let directory = corpus_dir(version);
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("corpus introuvable dans {} : {error}", directory.display()))
        .map(|entry| entry.expect("entrée de répertoire").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();

    assert!(
        !files.is_empty(),
        "aucun fichier .mscx dans le corpus {version}"
    );

    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut names = Vec::new();
    let mut identical = Vec::new();
    let mut rows = Vec::new();
    let mut diffs = String::new();

    for path in &files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier UTF-8")
            .to_owned();
        names.push(name.clone());

        let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
        let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
            panic!("référence {} illisible : {error}", reference_path.display())
        });
        let expected = normalize(&reference);

        let input = fs::read(path).expect("lecture du .mscx");
        let actual = match convert(&input, &options) {
            Ok(bytes) => normalize(&String::from_utf8(bytes).expect("MusicXML UTF-8")),
            Err(error) => {
                rows.push(format!("{name} : ERREUR ({error})"));
                diffs.push_str(&format!("\n=== {name} : ERREUR ===\n{error}\n"));
                continue;
            }
        };

        let (changed, diff) = diff(&expected, &actual);
        if changed == 0 {
            identical.push(name.clone());
            rows.push(format!("{name} : identique"));
        } else {
            rows.push(format!("{name} : {changed} lignes d'écart"));
            diffs.push_str(&format!("\n=== {name} : {changed} lignes d'écart ===\n"));
            diffs.push_str(&truncate_diff(&diff, 80));
            diffs.push('\n');
        }
    }

    println!("conformité du corpus {version} (après normalisation) :");
    for row in &rows {
        println!("  {row}");
    }
    if !diffs.is_empty() {
        println!("{diffs}");
    }

    for name in expected_conformant {
        assert!(
            names.iter().any(|candidate| candidate == name),
            "{name} est listé dans EXPECTED_CONFORMANT mais absent du corpus"
        );
        assert!(
            identical.iter().any(|candidate| candidate == name),
            "{name} est attendu conforme mais diffère de sa référence"
        );
    }
}
