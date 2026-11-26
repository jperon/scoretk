// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : tests de la portée manuscrite NABC en neumes
// MEI 5.1 (deuxième portée adiastématique), rédigés en 2026.
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

//! GABC NABC → MEI 5.1 (deuxième portée) → GABC.

use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::Extension;
use scoretk::{read, write, Format, Options};
use scoretk_mei_export::MeiOptions;

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("gabc");
    let mut paths: Vec<_> = fs::read_dir(root)
        .expect("Gregorio corpus")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "gabc"))
        .collect();
    paths.sort();
    paths
}

fn full_options() -> MeiOptions {
    MeiOptions {
        full_support: true,
        ..MeiOptions::default()
    }
}

#[test]
fn nabc_files_are_detected_and_carried_in_the_model() {
    let with_nabc: Vec<_> = corpus()
        .into_iter()
        .filter(|path| {
            fs::read_to_string(path)
                .expect("GABC")
                .contains("nabc-lines:")
        })
        .collect();
    assert_eq!(with_nabc.len(), 3, "three versioned Gregorio examples");
    for path in with_nabc {
        let source = fs::read(&path).expect("GABC source");
        let score = read(&source, Format::Gabc).expect("GABC read");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload");
        };
        let count: usize = gregorian.syllables.iter().map(|s| s.nabc.len()).sum();
        assert!(count > 0, "{} has NABC snippets", path.display());
    }
}

#[test]
fn nabc_output_is_a_second_unpitched_staff_linked_by_corresp() {
    for path in corpus() {
        let source = fs::read(&path).expect("GABC source");
        let score = read(&source, Format::Gabc).expect("GABC read");
        let has_nabc = matches!(&score.extensions.format, Some(Extension::Gregorian(g)) if g.syllables.iter().any(|s| !s.nabc.is_empty()));
        let mei = scoretk_mei_export::write_mei_with(&score, &full_options()).expect("MEI");
        if let (Some(dir), true) = (std::env::var_os("SCORETK_MEI_NABC_DUMP_DIR"), has_nabc) {
            let name = path.file_name().expect("file name");
            fs::write(Path::new(&dir).join(name), &mei).expect("dump");
        }
        if !has_nabc {
            assert!(!mei.contains("gabc-nabc"), "{}", path.display());
            assert_eq!(mei.matches("<staff ").count(), 1, "{}", path.display());
            continue;
        }
        assert!(mei.contains("<staffDef n=\"2\" notationtype=\"neume\" label=\"manuscript\">"));
        assert!(mei
            .lines()
            .any(|line| line.trim_start().starts_with("<staffDef n=\"2\"")
                && !line.contains("lines=")));
        assert!(mei.contains("<staff n=\"2\">"));
        assert!(mei.contains("type=\"gabc-nabc\""));
        assert!(mei.contains("corresp=\"#s1b\""));
        assert!(mei.contains("<neume type=\""));
        assert!(mei.contains("<nc "));
        assert!(
            mei.contains("intm=\""),
            "manuscript intervals are structured"
        );
    }
}

#[test]
fn gabc_to_mei_to_gabc_keeps_every_nabc_snippet_byte_for_byte() {
    let mut files = 0;
    let mut snippets = 0;
    for path in corpus() {
        let source = fs::read(&path).expect("GABC source");
        let score = read(&source, Format::Gabc).expect("GABC read");
        let Some(Extension::Gregorian(expected)) = &score.extensions.format else {
            panic!("L6 payload");
        };
        if !expected
            .syllables
            .iter()
            .any(|syllable| !syllable.nabc.is_empty())
        {
            continue;
        }
        files += 1;
        let mei = scoretk_mei_export::write_mei_with(&score, &full_options()).expect("MEI");
        let from_mei = read(mei.as_bytes(), Format::Mei).expect("MEI read");
        let Some(Extension::Gregorian(actual)) = &from_mei.extensions.format else {
            panic!("L6 payload after MEI");
        };
        assert_eq!(actual.syllables.len(), expected.syllables.len());
        for (index, (before, after)) in expected.syllables.iter().zip(&actual.syllables).enumerate()
        {
            assert_eq!(
                before.nabc,
                after.nabc,
                "{} syllable {index}: NABC snippets must cross MEI unchanged",
                path.display()
            );
            snippets += before.nabc.len();
        }
        let gabc = write(&from_mei, Format::Gabc, &Options::default()).expect("GABC write");
        // The MEI cross trip must produce the exact same bytes as today's
        // direct GABC → model → GABC serialisation, on every NABC file.
        let baseline = write(&score, Format::Gabc, &Options::default()).expect("GABC baseline");
        assert_eq!(
            gabc,
            baseline,
            "{}: MEI must not change GABC bytes",
            path.display()
        );
        let again = read(&gabc, Format::Gabc).expect("GABC re-read");
        let Some(Extension::Gregorian(rebuilt)) = &again.extensions.format else {
            panic!("L6 payload after round trip");
        };
        // UTF-16 source offsets point into the re-serialised GABC, whose
        // comments and layout differ from the input: they are the one field
        // with no MEI carrier (as in `gabc_mei_neumes.rs`).
        let original = read(&baseline, Format::Gabc).expect("baseline re-read");
        let Some(Extension::Gregorian(expected_syllables_source)) = &original.extensions.format
        else {
            panic!("baseline L6 payload");
        };
        let mut expected_syllables = expected_syllables_source.syllables.clone();
        let mut rebuilt_syllables = rebuilt.syllables.clone();
        for syllables in [&mut expected_syllables, &mut rebuilt_syllables] {
            for syllable in syllables {
                for note in &mut syllable.notes {
                    note.source_offset = None;
                    note.source_end = None;
                }
            }
        }
        if rebuilt_syllables != expected_syllables {
            for (index, (before, after)) in expected_syllables
                .iter()
                .zip(&rebuilt_syllables)
                .enumerate()
            {
                if before != after {
                    panic!(
                        "{} syllable {index} text {:?}→{:?}: notes {}→{}, bars {:?}→{:?}, nabc {:?}→{:?}",
                        path.display(), before.text, after.text, before.notes.len(), after.notes.len(),
                        before.bars, after.bars, before.nabc, after.nabc
                    );
                }
            }
        }
    }
    assert_eq!(files, 3);
    assert!(snippets > 50, "{snippets} NABC snippets measured");
    println!("GABC → MEI → GABC: {files} files, {snippets} NABC snippets byte-exact");
}

#[test]
fn nabc_structural_vs_textual_coverage_is_measured() {
    use scoretk_mei_common::nabc;

    let mut snippets = 0usize;
    let mut descriptors = 0usize;
    let mut structural = 0usize;
    let mut text_only = 0usize;
    let mut episemata = 0usize;
    let mut liquescents = 0usize;
    let mut letters_with_place = 0usize;
    let mut letters_without_place = 0usize;
    for path in corpus() {
        let source = fs::read(&path).expect("GABC source");
        let score = read(&source, Format::Gabc).expect("GABC read");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload");
        };
        for syllable in &gregorian.syllables {
            for snippet in &syllable.nabc {
                snippets += 1;
                for descriptor in nabc::parse(&snippet.text) {
                    descriptors += 1;
                    if descriptor.glyphs.is_empty() {
                        text_only += 1;
                    } else {
                        structural += 1;
                        for glyph in &descriptor.glyphs {
                            episemata += usize::from(glyph.modifiers.contains('-'));
                            liquescents += usize::from(
                                glyph.modifiers.contains('>') || glyph.modifiers.contains('~'),
                            );
                        }
                    }
                    for (_, position) in &descriptor.letters {
                        if nabc::place(*position).is_some() {
                            letters_with_place += 1;
                        } else {
                            letters_without_place += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(descriptors > 0);
    let structural_pct = format!("{:.2}%", 100.0 * structural as f64 / descriptors as f64);
    let text_pct = format!("{:.2}%", 100.0 * text_only as f64 / descriptors as f64);
    println!(
        "NABC coverage: {snippets} snippets, {descriptors} descriptors, \
         {structural} structural ({structural_pct}), {text_only} text-only ({text_pct}), \
         {episemata} episemata, {liquescents} liquescences, \
         {letters_with_place}/{letters_without_place} significant letters with/without @place"
    );
    assert_eq!(structural + text_only, descriptors);
}

#[test]
fn default_mei_output_is_unchanged_without_full_support() {
    for path in corpus() {
        let source = fs::read(&path).expect("GABC source");
        let score = read(&source, Format::Gabc).expect("GABC read");
        let default = write(&score, Format::Mei, &Options::default()).expect("MEI");
        assert!(!String::from_utf8_lossy(&default).contains("gabc-nabc"));
        assert_eq!(
            String::from_utf8_lossy(&default).matches("<staff ").count(),
            1
        );
    }
}
