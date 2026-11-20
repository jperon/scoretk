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
//! The test prints a table "file: identical / N differing lines" and
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

/// Files whose conversion is currently identical to the reference after
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

/// Files of the 3.x corpus whose conversion is identical to the reference
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
    /// Number of differing lines per file, in alphabetical order.
    rows: Vec<(String, usize)>,
    /// Discrepancies grouped by tag name, with examples.
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
                    .push_str(&format!("\n=== {version}/{name}: ERROR ===\n{error}\n"));
                continue;
            }
        };

        let (changed, diff) = diff(&expected, &actual);
        report.rows.push((name.clone(), changed));
        if changed > 0 {
            collect_tags(&expected, &actual, &name, &mut report.tags);
            report.diffs.push_str(&format!(
                "\n=== {version}/{name}: {changed} differing lines ===\n"
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

/// Prints the table and the grouped discrepancies of a corpus.
fn print_full_report(version: &str, report: &FullReport) {
    println!("full compliance of corpus {version} (pure layout removed):");
    for (name, changed) in &report.rows {
        if *changed == usize::MAX {
            println!("  {name}: ERROR");
        } else if *changed == 0 {
            println!("  {name}: identical");
        } else {
            println!("  {name}: {changed} differing lines");
        }
    }
    println!("discrepancies grouped by tag:");
    print_tag_stats(&report.tags);
    println!("{}", report.diffs);
}

/// Prints grouped discrepancies, sorted by decreasing line count.
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
            examples.push_str("(no example)");
        }
        println!(
            "  {tag}: {} lines, {} files — {examples}",
            stat.lines,
            stat.files.len()
        );
    }
}

/// Produces the contents of `docs/agents/ecarts-complets.md`.
///
/// The test is deliberately non-blocking: it measures the full discrepancy to
/// track parity, without requiring it to be reached.
#[test]
fn ecarts_complets_document() {
    let v4 = run_full_corpus("v4");
    let v3 = run_full_corpus("v3");

    println!("---DEBUT DOC---");
    println!("# Full discrepancies on the corpus (full parity)");
    println!();
    println!(
        "This document is produced by the `ecarts_complets_document` test in \
         `crates/scoretk/tests/e2e_corpus.rs`. It compares each `.mscx` to its \
         `X.ref.musicxml` reference after removing **pure layout** \
         only: attributes `default-x`, `default-y`, `relative-x`, \
         `relative-y`, `width`, `<defaults>`, `<print>` blocks (and their \
         `<system-layout>`/`<staff-layout>`) and `<encoding-date>`. Everything \
         else is compared: `<stem>`, `<beam>`, `<barline>`, `<direction>`, \
         `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc."
    );
    println!();
    println!("## Table by file");
    println!();
    for (version, report) in [("v3", &v3), ("v4", &v4)] {
        println!("### Corpus {version}");
        println!();
        println!("| File | Differing lines |");
        println!("| --- | ---: |");
        for (name, changed) in &report.rows {
            if *changed == usize::MAX {
                println!("| {name} | ERROR |");
            } else if *changed == 0 {
                println!("| {name} | **identical** |");
            } else {
                println!("| {name} | {changed} |");
            }
        }
        println!();
    }

    println!("## Discrepancies grouped by tag");
    println!();
    println!(
        "Frequency = number of modified canonical lines across both corpora; \
              files = number of affected files. Sorted by decreasing frequency."
    );
    println!();
    println!("| Tag | Lines | Files | Examples |");
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
    println!("## Discrepancies that depend on computed layout");
    println!();
    println!(
        "Some discrepancies do not come from a datum of the `.mscx` but from an object \
         that MuseScore builds during layout, absent from the model. \
         They are irreducible without porting the layout engine:\n\n\
         - **stem direction of a cross beam** (`v3/moonlight`, \
           9 notes): `BeamLayout::layout1` sets the beam direction from \
           `computeUpForMovedCross` and `isBelowCrossBeam`, hence from beam \
           grouping and note geometry, not from the file's \
           `<Beam><StemDirection>`;\n\
         - **courtesy barlines of a system cut by page width** \
           (`v3/voices`, 2 `light-light` barlines): `MeasureLayout::createEndBarLines` \
           places the double barline only if the measure is the last in the system \
           (`isLastMeasureInSystem`) and the next one changes key signature. When \
           the system break appears in the file (`<LayoutBreak>`), it is \
           reproduced (see `export/systems.rs` and `read/measure_repeat.rs`); \
           an automatic cut depends on the computed width of measures, \
           hence on the whole horizontal layout."
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
                rows.push(format!("{name}: ERROR ({error})"));
                diffs.push_str(&format!("\n=== {name}: ERROR ===\n{error}\n"));
                continue;
            }
        };

        let (changed, diff) = diff(&expected, &actual);
        if changed == 0 {
            identical.push(name.clone());
            rows.push(format!("{name}: identical"));
        } else {
            rows.push(format!("{name}: {changed} differing lines"));
            diffs.push_str(&format!("\n=== {name}: {changed} differing lines ===\n"));
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
