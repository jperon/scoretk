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

//! Interoperability measurement without scoretk-private carriers.
//!
//! The writers of ABC and LilyPond may emit private syntax that only scoretk's
//! own readers interpret. `docs/agents/model-modularisation.md` §11.8 measures
//! what a *different* tool would recover: the written file is stripped of its
//! private carriers before being re-read, with the same declared layers and
//! the same `(layer, tick, staff, kind, value)` inventory as `layer_coverage`.
//!
//! This test versions that formerly temporary harness and prints the full
//! per-kind loss table. It never asserts on a percentage: it is a diagnostic,
//! like `layer_coverage` and `midi_full_loss_table`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::{compare, inventory, LayerCounts};
use scoretk::{read, write, Format, Options, Score};

use scoretk_abc_export::CAPABILITIES as ABC_EXPORT;
use scoretk_abc_import::CAPABILITIES as ABC_IMPORT;
use scoretk_lilypond_export::CAPABILITIES as LY_EXPORT;
use scoretk_lilypond_import::CAPABILITIES as LY_IMPORT;

#[derive(Default)]
struct Totals {
    files: usize,
    skipped: usize,
    total: LayerCounts,
    missing: BTreeMap<(u8, &'static str), usize>,
}

fn corpus_scores() -> Vec<(String, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    let mut scores = Vec::new();
    for version in ["v3", "v4"] {
        let mut paths: Vec<PathBuf> = fs::read_dir(root.join(version))
            .expect("corpus directory")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_stem()
                .expect("score file name")
                .to_string_lossy()
                .into_owned();
            scores.push((format!("{version}/{name}"), path));
        }
    }
    scores
}

fn reference_model(path: &Path) -> Score {
    let bytes = fs::read(path).expect("read .mscx reference");
    read(&bytes, Format::Mscx).expect("MuseScore import of the reference")
}

/// Removes the private ABC carriers of §11.8: `%%scoretk …` directives and
/// `!scoretk-…!` decorations. Every other character is kept byte for byte.
fn strip_abc_private(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.split_inclusive('\n') {
        if line.trim_start().starts_with("%%scoretk") {
            continue;
        }
        let mut rest = line;
        loop {
            let Some(at) = rest.find("!scoretk-") else {
                out.push_str(rest);
                break;
            };
            out.push_str(&rest[..at]);
            let Some(end) = rest[at + 1..].find('!') else {
                // An unclosed decoration is not a private carrier: keep it.
                out.push_str(&rest[at..]);
                break;
            };
            rest = &rest[at + 1 + end + 1..];
        }
    }
    out
}

/// Header fields not in LilyPond's default `\header` field list (Notation
/// Reference §21.1.2), plus the private `scoretk*` fields.
const LY_PRIVATE_HEADER_FIELDS: &[&str] = &[
    "workTitle",
    "movementTitle",
    "movementNumber",
    "workNumber",
    "source",
];

/// Removes the private LilyPond carriers of §11.8: non-standard `\header`
/// fields and `^\markup { "scoretk-…" }` / `_\markup { "scoretk-…" }` texts.
fn strip_lilypond_private(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_header = false;
    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("\\header") {
            in_header = true;
            out.push_str(line);
            continue;
        }
        if in_header {
            if trimmed.starts_with('}') {
                in_header = false;
            } else if let Some((name, _)) = trimmed.split_once('=') {
                let name = name.trim();
                if name.starts_with("scoretk") || LY_PRIVATE_HEADER_FIELDS.contains(&name) {
                    continue;
                }
            }
        }
        let mut rest = line;
        loop {
            let Some(at) = rest.find("\"scoretk-") else {
                out.push_str(rest);
                break;
            };
            // Remove the whole markup, including its `^`/`_` placement.
            let prefix = &rest[..at];
            let Some(markup) = prefix.rfind("\\markup") else {
                // A private string outside a markup is not a carrier here.
                out.push_str(rest);
                break;
            };
            let mut start = markup;
            if start > 0 && matches!(prefix.as_bytes()[start - 1], b'^' | b'_') {
                start -= 1;
            }
            out.push_str(&rest[..start]);
            let Some(end) = rest[at..].find("\" }") else {
                out.push_str(&rest[start..]);
                break;
            };
            rest = &rest[at + end + 3..];
        }
    }
    out
}

fn measure(format: Format, strip: Option<fn(&str) -> String>) -> Totals {
    let declared = match format {
        Format::Abc => ABC_IMPORT.declared().union(ABC_EXPORT.declared()),
        Format::Ly => LY_IMPORT.declared().union(LY_EXPORT.declared()),
        _ => unreachable!("only ABC and LilyPond are measured here"),
    };
    let kinds = match format {
        Format::Abc => ABC_IMPORT.declared_kinds().or(ABC_EXPORT.declared_kinds()),
        Format::Ly => LY_IMPORT.declared_kinds().or(LY_EXPORT.declared_kinds()),
        _ => unreachable!(),
    };
    let mut totals = Totals::default();
    for (_, path) in corpus_scores() {
        let score = reference_model(&path);
        let reference = inventory(&score);
        let Ok(bytes) = write(&score, format, &Options::default()) else {
            totals.skipped += 1;
            continue;
        };
        let bytes = match strip {
            Some(strip) => strip(&String::from_utf8(bytes).expect("UTF-8 output")).into_bytes(),
            None => bytes,
        };
        let Ok(imported) = read(&bytes, format) else {
            totals.skipped += 1;
            continue;
        };
        let coverage = compare(&reference, &inventory(&imported), declared);
        let _ = kinds; // ABC and LilyPond declare whole layers, not finer kinds.
        assert_eq!(
            coverage.total.reference,
            reference
                .items
                .iter()
                .filter(|item| declared.contains(item.layer))
                .count()
        );
        totals.files += 1;
        totals.total.reference += coverage.total.reference;
        totals.total.matched += coverage.total.matched;
        totals.total.extra += coverage.total.extra;
        for ((bits, kind), count) in coverage.missing_kinds {
            *totals.missing.entry((bits, kind)).or_insert(0) += count;
        }
    }
    totals
}

fn print_table(label: &str, totals: &Totals, stripped: bool) {
    println!(
        "\n=== {label} ({} files, {} skipped, {:.2} % = {}/{}) ===",
        totals.files,
        totals.skipped,
        totals.total.percent(),
        totals.total.matched,
        totals.total.reference,
    );
    let mut gaps: Vec<((u8, &'static str), usize)> = totals
        .missing
        .iter()
        .map(|(key, count)| (*key, *count))
        .collect();
    gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for ((bits, kind), count) in gaps {
        println!(
            "    {:<28} L{}  {:>5}{}",
            kind,
            bits,
            count,
            if stripped { "  [stripped]" } else { "" }
        );
    }
}

#[test]
fn abc_per_kind_losses_with_and_without_private_extensions() {
    let private = measure(Format::Abc, None);
    let stripped = measure(Format::Abc, Some(strip_abc_private));
    print_table(
        "ABC per-kind losses, private carriers kept",
        &private,
        false,
    );
    print_table(
        "ABC per-kind losses, private carriers stripped (standard ABC only)",
        &stripped,
        true,
    );
    println!();
}

#[test]
fn lilypond_per_kind_losses_with_and_without_private_extensions() {
    let private = measure(Format::Ly, None);
    let stripped = measure(Format::Ly, Some(strip_lilypond_private));
    print_table(
        "LilyPond per-kind losses, private carriers kept",
        &private,
        false,
    );
    print_table(
        "LilyPond per-kind losses, private carriers stripped (standard LilyPond only)",
        &stripped,
        true,
    );
    println!();
}

/// Prints the lost reference values of selected kinds, aggregated over the
/// corpus, so the standard carrier can be designed from real items rather than
/// from a kind count. Diagnostic only.
fn print_lost_values(format: Format, strip: Option<fn(&str) -> String>, kinds: &[&str]) {
    let declared = match format {
        Format::Abc => ABC_IMPORT.declared().union(ABC_EXPORT.declared()),
        Format::Ly => LY_IMPORT.declared().union(LY_EXPORT.declared()),
        _ => unreachable!(),
    };
    let mut lost: BTreeMap<&'static str, BTreeMap<String, usize>> = BTreeMap::new();
    for (name, path) in corpus_scores() {
        let score = reference_model(&path);
        let reference = inventory(&score);
        let Ok(bytes) = write(&score, format, &Options::default()) else {
            continue;
        };
        let bytes = match strip {
            Some(strip) => strip(&String::from_utf8(bytes).expect("UTF-8 output")).into_bytes(),
            None => bytes,
        };
        let Ok(imported) = read(&bytes, format) else {
            continue;
        };
        let _ = name;
        let actual = inventory(&imported);
        for item in &reference.items {
            if !declared.contains(item.layer) || !kinds.contains(&item.kind) {
                continue;
            }
            let wanted = reference
                .items
                .iter()
                .filter(|other| {
                    other.layer == item.layer
                        && other.kind == item.kind
                        && other.value == item.value
                })
                .count();
            let found = actual
                .items
                .iter()
                .filter(|other| {
                    other.layer == item.layer
                        && other.kind == item.kind
                        && other.value == item.value
                })
                .count();
            if found < wanted {
                *lost
                    .entry(item.kind)
                    .or_default()
                    .entry(item.value.clone())
                    .or_insert(0) += 1;
            }
        }
    }
    for (kind, values) in lost {
        println!(
            "\n--- {format:?} lost {kind} values (stripped: {}) ---",
            strip.is_some()
        );
        for (value, count) in values {
            println!("    {count:>4}  {value}");
        }
    }
}

#[test]
fn abc_private_loss_values() {
    print_lost_values(
        Format::Abc,
        Some(strip_abc_private),
        &[
            "instrument",
            "dynamic-velocity",
            "title",
            "tbox-text",
            "tremolo",
        ],
    );
    println!();
}

#[test]
fn lilypond_private_loss_values() {
    print_lost_values(
        Format::Ly,
        Some(strip_lilypond_private),
        &[
            "style",
            "title",
            "credit",
            "credit-position",
            "tbox-text",
            "tbox-position",
            "note-head",
            "tremolo",
        ],
    );
    println!();
}
