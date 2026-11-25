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

use scoretk::model::{compare_with_kinds, inventory, LayerCounts, LayerInventory};
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

/// Retire les seules valeurs `note-head` sans style LilyPond univoque
/// (`docs/agents/lilypond.md` §52.3). Le genre reste dans la déclaration et
/// toutes les autres valeurs sont comparées normalement, des deux côtés.
fn declared_inventory(score: &Score) -> LayerInventory {
    let mut result = inventory(score);
    result.items.retain(|item| match item.kind {
        "note-head" => !scoretk_lilypond_import::noteheads::out_of_reach(&item.value),
        "tremolo" => !scoretk_lilypond_import::tremolo_out_of_reach(&item.value),
        "time-sig" => !scoretk_lilypond_import::time_sig_out_of_reach(&item.value),
        "style" => !scoretk_lilypond_import::style_out_of_reach(&item.value),
        _ => true,
    });
    result
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
        let reference = declared_inventory(&score);
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
        let coverage = compare_with_kinds(&reference, &inventory(&imported), declared, kinds);
        assert_eq!(
            coverage.total.reference,
            reference
                .items
                .iter()
                .filter(|item| {
                    declared.contains(item.layer)
                        && kinds.is_none_or(|kinds| kinds.contains(&item.kind))
                })
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

/// The ABC figures after the standard carriers were added. Both rows are
/// measured on the same 56 scores with the same declared layers; only the
/// private carriers are deleted before re-reading.
#[test]
fn abc_full_support_reaches_target_with_and_without_private_extensions() {
    let private = measure(Format::Abc, None);
    let stripped = measure(Format::Abc, Some(strip_abc_private));
    assert_eq!(private.files, 56);
    assert_eq!(private.skipped, 0);
    assert_eq!(stripped.files, 56);
    assert_eq!(stripped.skipped, 0);
    assert!(
        private.total.percent() >= 95.0,
        "ABC with private carriers: {:.2} %",
        private.total.percent()
    );
    assert!(
        stripped.total.percent() >= 95.0,
        "ABC without private carriers: {:.2} %",
        stripped.total.percent()
    );
    println!(
        "ABC full support: {:.2} % with private carriers, {:.2} % without ({}/{})",
        private.total.percent(),
        stripped.total.percent(),
        stripped.total.matched,
        stripped.total.reference,
    );
}

/// The declaration is about what LilyPond 2.26 can carry, not a relaxation of
/// the oracle. Frame, credit-position, chord-symbol and staff/system-text
/// objects have no LilyPond carrier (§6.1, §8.2, §21.1.2); `note-head` and
/// `tremolo` **are** declared, with only the individual head groups and the
/// buzz roll excluded per value by `noteheads::out_of_reach` and
/// `tremolo_out_of_reach` (§B.9 and §2.5.3).
#[test]
fn lilypond_declaration_follows_notation_reference() {
    let kinds = scoretk_lilypond_import::CAPABILITIES
        .declared_kinds()
        .expect("the LilyPond declaration lists the carried kinds");
    for kind in [
        "tbox-position",   // no MuseScore frame in LilyPond (§6.1, §8.2)
        "tbox-text",       // idem
        "credit-position", // no per-field position in \header (§21.1.2)
        "harmony",         // no per-tick <harmony> object; \markup is text
        "system-text",     // \markup has no staff/system distinction (§8.2)
        "instrument",      // MuseScore id, not a General MIDI name (§24.5)
        "channel",         // no per-instrument MIDI channel (§24.8)
    ] {
        assert!(!kinds.contains(&kind), "{kind} has no LilyPond carrier");
    }
    for kind in [
        "note-head", // `\tweak style #'cross|harmonic|do|…` (§B.9)
        "tremolo",   // `:8` and `\repeat tremolo n` (§2.5.3)
    ] {
        assert!(kinds.contains(&kind), "{kind} is carried by LilyPond");
    }
    for kind in [
        "note",
        "measure",
        "rest",
        "title",
        "credit",
        "style",
        "tempo",
        "dynamic-velocity",
        "program",
        "line-break",
    ] {
        assert!(kinds.contains(&kind), "{kind} is carried by LilyPond");
    }
    assert_eq!(
        scoretk_lilypond_import::CAPABILITIES.declared_kinds(),
        scoretk_lilypond_export::CAPABILITIES.declared_kinds()
    );
}

/// LilyPond figures after the standard carriers were added and the three
/// uncarriable kinds were declared out of reach with their spec evidence.
#[test]
fn lilypond_full_support_reaches_target_with_and_without_private_extensions() {
    let private = measure(Format::Ly, None);
    let stripped = measure(Format::Ly, Some(strip_lilypond_private));
    assert_eq!(private.files, 56);
    assert_eq!(private.skipped, 0);
    assert_eq!(stripped.files, 56);
    assert_eq!(stripped.skipped, 0);
    assert!(
        private.total.percent() >= 95.0,
        "LilyPond with private carriers: {:.2} %",
        private.total.percent()
    );
    assert!(
        stripped.total.percent() >= 95.0,
        "LilyPond without private carriers: {:.2} %",
        stripped.total.percent()
    );
    println!(
        "LilyPond full support: {:.2} % with private carriers, {:.2} % without ({}/{})",
        private.total.percent(),
        stripped.total.percent(),
        stripped.total.matched,
        stripped.total.reference,
    );
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
        let reference = declared_inventory(&score);
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
        let actual = declared_inventory(&imported);
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

/// Multiset-exact diagnostic: prints the lost keys (tick, staff, value) of
/// selected kinds, one line per missing reference item.
fn print_lost_keys(format: Format, strip: Option<fn(&str) -> String>, kinds: &[&str]) {
    use std::collections::BTreeMap;
    let declared = match format {
        Format::Abc => ABC_IMPORT.declared().union(ABC_EXPORT.declared()),
        Format::Ly => LY_IMPORT.declared().union(LY_EXPORT.declared()),
        _ => unreachable!(),
    };
    let mut missing: BTreeMap<String, usize> = BTreeMap::new();
    for (name, path) in corpus_scores() {
        let score = reference_model(&path);
        let reference = declared_inventory(&score);
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
        let actual = declared_inventory(&imported);
        let key = |item: &scoretk::model::InventoryItem| {
            (
                item.layer.bits(),
                item.tick,
                item.staff,
                item.kind,
                item.value.clone(),
            )
        };
        let mut wanted: BTreeMap<_, usize> = BTreeMap::new();
        let mut found: BTreeMap<_, usize> = BTreeMap::new();
        for item in &reference.items {
            if declared.contains(item.layer) && kinds.contains(&item.kind) {
                *wanted.entry(key(item)).or_insert(0) += 1;
            }
        }
        for item in &actual.items {
            if declared.contains(item.layer) && kinds.contains(&item.kind) {
                *found.entry(key(item)).or_insert(0) += 1;
            }
        }
        for (key, count) in wanted {
            let got = found.get_mut(&key).map_or(0, |slot| {
                let taken = (*slot).min(count);
                *slot -= taken;
                taken
            });
            for _ in got..count {
                *missing
                    .entry(format!(
                        "{name} {} tick {:?} staff {:?} {}",
                        key.3, key.1, key.2, key.4
                    ))
                    .or_insert(0) += 1;
            }
        }
    }
    println!("\n--- {format:?} missing keys ---");
    for (key, count) in missing {
        println!("    {count:>4}  {key}");
    }
}

#[test]
fn abc_missing_private_keys() {
    print_lost_keys(
        Format::Abc,
        Some(strip_abc_private),
        &[
            "instrument",
            "dynamic-velocity",
            "title",
            "marker",
            "jump",
            "harmony",
            "repeat-count",
            "rehearsal-mark",
        ],
    );
    println!();
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
            "tempo",
            "harmony",
            "rehearsal-mark",
            "system-text",
            "text-line",
            "fermata",
            "dynamic",
            "rest",
        ],
    );
    println!();
}

#[test]
fn lilypond_missing_credit_keys() {
    print_lost_keys(
        Format::Ly,
        Some(strip_lilypond_private),
        &[
            "credit",
            "note-head",
            "dynamic-velocity",
            "instrument",
            "program",
            "fermata",
            "system-text",
            "line-break",
            "harmony",
            "dynamic",
        ],
    );
    println!();
}

#[test]
fn lilypond_other_loss_keys() {
    print_lost_keys(
        Format::Ly,
        Some(strip_lilypond_private),
        &[
            "rest",
            "measure",
            "clef",
            "marker",
            "lyric",
            "tuplet",
            "jump",
            "fermata",
            "system-text",
            "text-line",
            "dynamic",
            "time-sig",
            "harmony",
            "tbox-text",
            "credit-position",
            "tempo",
            "line-break",
            "arpeggio",
            "glissando",
            "pedal",
            "program",
            "instrument",
            "dynamic-velocity",
            "volume",
            "pan",
            "barline-end",
            "rehearsal-mark",
            "credit",
            "articulation",
            "volta",
        ],
    );
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
            "tempo",
            "harmony",
            "rest",
        ],
    );
    println!();
}
