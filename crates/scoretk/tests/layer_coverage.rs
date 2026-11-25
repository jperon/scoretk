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

//! Layer coverage of the ten directions, measured on the MuseScore corpus.
//!
//! Run with `--nocapture`. This test **never fails on a percentage**: it
//! prints a measurement table and only panics on a broken corpus/setup
//! (unreadable reference score). A direction that cannot write or read a file
//! is counted as `skipped` and printed, not asserted on.
//!
//! Definitions (the same as in `docs/agents/model-modularisation.md` §11):
//!
//! - an item is keyed by `(layer, tick, staff, kind, value)` and compared as a
//!   multiset, so a moved or changed item is not silently counted as present;
//! - **essential** = matched `note` + `measure` items / reference items of
//!   those two kinds (today's musical-core comparison);
//! - **full support** = matched items / reference items, restricted to the
//!   layers the format declares;
//! - **per-layer** = the same ratio for one declared layer; a layer with no
//!   reference item is 100 % by convention and is not printed;
//! - reference model = the `.mscx` read by MuseScore import. The round trip is
//!   `model → write X → read X`; the import-alone rows read an existing
//!   reference file (`*.ref.musicxml`, `*.ref.mid`, or the `.ly`/`.abc` corpus
//!   snapshots) and compare it with that same reference model.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::{
    compare, essential, inventory, Capabilities, Coverage, LayerCounts, Layers, LAYERS,
};
use scoretk::{read, write, Format, Options, Score};

use scoretk_abc_export::CAPABILITIES as ABC_EXPORT;
use scoretk_abc_import::CAPABILITIES as ABC_IMPORT;
use scoretk_lilypond_export::CAPABILITIES as LY_EXPORT;
use scoretk_lilypond_import::CAPABILITIES as LY_IMPORT;
use scoretk_midi_export::CAPABILITIES as MIDI_EXPORT;
use scoretk_midi_import::CAPABILITIES as MIDI_IMPORT;
use scoretk_musescore_export::CAPABILITIES as MSCX_EXPORT;
use scoretk_musescore_import::CAPABILITIES as MSCX_IMPORT;
use scoretk_musicxml_export::CAPABILITIES as XML_EXPORT;
use scoretk_musicxml_import::CAPABILITIES as XML_IMPORT;

/// One measured export direction (a format with both a reader and a writer).
struct Direction {
    name: &'static str,
    format: Format,
    import: Capabilities,
    export: Capabilities,
    /// Kind of external reference file, when one exists for this direction.
    reference: Option<ReferenceKind>,
}

#[derive(Clone, Copy)]
enum ReferenceKind {
    MusicXml,
    Midi,
    Ly,
    Abc,
}

const DIRECTIONS: &[Direction] = &[
    Direction {
        name: "mscx",
        format: Format::Mscx,
        import: MSCX_IMPORT,
        export: MSCX_EXPORT,
        reference: None,
    },
    Direction {
        name: "mscz",
        format: Format::Mscz,
        import: MSCX_IMPORT,
        export: MSCX_EXPORT,
        reference: None,
    },
    Direction {
        name: "musicxml",
        format: Format::MusicXml,
        import: XML_IMPORT,
        export: XML_EXPORT,
        reference: Some(ReferenceKind::MusicXml),
    },
    Direction {
        name: "mxl",
        format: Format::Mxl,
        import: XML_IMPORT,
        export: XML_EXPORT,
        reference: Some(ReferenceKind::MusicXml),
    },
    Direction {
        name: "midi",
        format: Format::Midi,
        import: MIDI_IMPORT,
        export: MIDI_EXPORT,
        reference: Some(ReferenceKind::Midi),
    },
    Direction {
        name: "lilypond",
        format: Format::Ly,
        import: LY_IMPORT,
        export: LY_EXPORT,
        reference: Some(ReferenceKind::Ly),
    },
    Direction {
        name: "abc",
        format: Format::Abc,
        import: ABC_IMPORT,
        export: ABC_EXPORT,
        reference: Some(ReferenceKind::Abc),
    },
];

/// Accumulated counts over the whole corpus, for one direction.
#[derive(Default)]
struct Totals {
    files: usize,
    skipped: usize,
    essential: LayerCounts,
    total: LayerCounts,
    per_layer: BTreeMap<u8, LayerCounts>,
    missing_kinds: BTreeMap<(u8, &'static str), usize>,
}

impl Totals {
    fn add(&mut self, coverage: &Coverage, essential: LayerCounts) {
        self.files += 1;
        self.essential.reference += essential.reference;
        self.essential.matched += essential.matched;
        self.essential.extra += essential.extra;
        self.total.reference += coverage.total.reference;
        self.total.matched += coverage.total.matched;
        self.total.extra += coverage.total.extra;
        for (bits, count) in &coverage.per_layer {
            let accumulated = self.per_layer.entry(*bits).or_default();
            accumulated.reference += count.reference;
            accumulated.matched += count.matched;
            accumulated.extra += count.extra;
        }
        for ((bits, kind), count) in &coverage.missing_kinds {
            *self.missing_kinds.entry((*bits, *kind)).or_insert(0) += count;
        }
    }
}

fn layers_label(layers: Layers) -> String {
    if layers == Layers::ALL {
        return "L0-6".to_owned();
    }
    let mut label = String::new();
    for (layer, name) in [
        (Layers::COMMON, "L0"),
        (Layers::METRIC, "L1"),
        (Layers::EVENTS, "L2"),
        (Layers::TEXT, "L3"),
        (Layers::PLAYBACK, "L4"),
        (Layers::ENGRAVING, "L5"),
        (Layers::EXTENSIONS, "L6"),
    ] {
        if layers.contains(layer) {
            label.push_str(name);
        }
    }
    if label.is_empty() {
        "none".to_owned()
    } else {
        label
    }
}

fn layer_name(bits: u8) -> &'static str {
    match bits {
        bits if bits == Layers::COMMON.bits() => "L0 common",
        bits if bits == Layers::METRIC.bits() => "L1 metric",
        bits if bits == Layers::EVENTS.bits() => "L2 events",
        bits if bits == Layers::TEXT.bits() => "L3 text",
        bits if bits == Layers::PLAYBACK.bits() => "L4 playback",
        bits if bits == Layers::ENGRAVING.bits() => "L5 engraving",
        bits if bits == Layers::EXTENSIONS.bits() => "L6 extensions",
        _ => "mixed",
    }
}

fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
}

/// `(version/name, path)` of every `.mscx` of the versioned corpus.
fn corpus_scores() -> Vec<(String, PathBuf)> {
    let mut scores = Vec::new();
    for version in ["v3", "v4"] {
        let mut paths: Vec<PathBuf> = fs::read_dir(corpus_dir(version))
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

fn reference_file(scores: &str, kind: ReferenceKind) -> Option<(PathBuf, Format)> {
    let (version, name) = scores.split_once('/').expect("version/name");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    let (path, format) = match kind {
        ReferenceKind::MusicXml => (
            corpus_dir(version).join(format!("{name}.ref.musicxml")),
            Format::MusicXml,
        ),
        ReferenceKind::Midi => (
            root.join("midi")
                .join(version)
                .join(format!("{name}.ref.mid")),
            Format::Midi,
        ),
        ReferenceKind::Ly => (
            root.join("ly").join(format!("{version}_{name}.ly")),
            Format::Ly,
        ),
        ReferenceKind::Abc => (
            root.join("abc").join(format!("{version}_{name}.abc")),
            Format::Abc,
        ),
    };
    path.is_file().then_some((path, format))
}

fn print_row(name: &str, declared: Layers, totals: &Totals) {
    println!(
        "{:<14} {:<12} {:>9.2}% {:>12.2}% {:>6} {:>7} {:>8}",
        name,
        layers_label(declared),
        totals.essential.percent(),
        totals.total.percent(),
        totals.files,
        totals.skipped,
        totals.total.reference,
    );
}

fn print_details(declared: Layers, totals: &Totals) {
    for layer in LAYERS {
        if !declared.contains(layer) {
            continue;
        }
        let count = totals
            .per_layer
            .get(&layer.bits())
            .copied()
            .unwrap_or_default();
        if count.reference == 0 {
            continue;
        }
        println!(
            "    {:<14} {:>6.2}%  {:>7}/{:<7} extra {}",
            layer_name(layer.bits()),
            count.percent(),
            count.matched,
            count.reference,
            count.extra,
        );
    }
    let mut gaps: Vec<((u8, &'static str), usize)> = totals
        .missing_kinds
        .iter()
        .map(|((bits, kind), count)| ((*bits, *kind), *count))
        .collect();
    gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let gaps: Vec<String> = gaps
        .into_iter()
        .take(8)
        .map(|((bits, kind), count)| format!("{kind} ({}): {count}", layer_name(bits)))
        .collect();
    if !gaps.is_empty() {
        println!("    largest gaps: {}", gaps.join(", "));
    }
}

#[test]
fn layer_coverage_table() {
    let scores = corpus_scores();
    println!(
        "\n=== Layer coverage on the MuseScore corpus ({} scores) ===",
        scores.len()
    );
    println!(
        "key = (layer, tick, staff, kind, value); essential = note+measure; \
         full support = all declared layers"
    );
    println!(
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}",
        "direction", "declared", "essential", "full-support", "files", "skipped", "ref-items"
    );

    for direction in DIRECTIONS {
        let declared = direction
            .import
            .declared()
            .union(direction.export.declared());
        let mut totals = Totals::default();
        for (_, path) in &scores {
            let reference = reference_model(path);
            let reference_inventory = inventory(&reference);
            let Ok(bytes) = write(&reference, direction.format, &Options::default()) else {
                totals.skipped += 1;
                continue;
            };
            let Ok(round_trip) = read(&bytes, direction.format) else {
                totals.skipped += 1;
                continue;
            };
            let candidate = inventory(&round_trip);
            let coverage = compare(&reference_inventory, &candidate, declared);
            totals.add(&coverage, essential(&reference_inventory, &candidate));
        }
        print_row(direction.name, declared, &totals);
        print_details(declared, &totals);
    }

    println!("\n=== Import alone against the same reference model ===");
    println!(
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}",
        "format", "declared", "essential", "full-support", "files", "skipped", "ref-items"
    );
    for direction in DIRECTIONS {
        let Some(kind) = direction.reference else {
            continue;
        };
        let declared = direction.import.declared();
        let mut totals = Totals::default();
        for (score, path) in &scores {
            let Some((reference_path, format)) = reference_file(score, kind) else {
                continue;
            };
            let reference = reference_model(path);
            let reference_inventory = inventory(&reference);
            let Ok(bytes) = fs::read(&reference_path) else {
                totals.skipped += 1;
                continue;
            };
            let Ok(imported) = read(&bytes, format) else {
                totals.skipped += 1;
                continue;
            };
            let candidate = inventory(&imported);
            let coverage = compare(&reference_inventory, &candidate, declared);
            totals.add(&coverage, essential(&reference_inventory, &candidate));
        }
        print_row(direction.name, declared, &totals);
        print_details(declared, &totals);
    }
    println!();
}
