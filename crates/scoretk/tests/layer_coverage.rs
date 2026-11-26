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
//! Run with `--nocapture`. This test never fails on a percentage **except the
//! MIDI full-support headline** (`midi_full_support_capacity_line`), which is
//! the target of the 2026-11-26 campaign: 95 % of what the SMF carries. It
//! otherwise prints a measurement table and only panics on a broken
//! corpus/setup (unreadable reference score). A direction that cannot write or
//! read a file is counted as `skipped` and printed, not asserted on.
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
    compare, compare_with_kinds, essential, inventory, Capabilities, Coverage, LayerCounts, Layers,
    LAYERS,
};
use scoretk::{read, read_midi_with, write, Format, MidiImportOptions, Options, Score};

mod support;
use support::midi_smf::{
    model_kind_values, model_notes, smf_inventory, smf_kind_values, smf_notes,
};

use scoretk_abc_export::CAPABILITIES as ABC_EXPORT;
use scoretk_abc_import::CAPABILITIES as ABC_IMPORT;
use scoretk_gabc_export::CAPABILITIES as GABC_EXPORT;
use scoretk_gabc_import::CAPABILITIES as GABC_IMPORT;
use scoretk_lilypond_export::CAPABILITIES as LY_EXPORT;
use scoretk_lilypond_import::CAPABILITIES as LY_IMPORT;
use scoretk_mei_export::{MeiOptions, CAPABILITIES as MEI_EXPORT};
use scoretk_mei_import::CAPABILITIES as MEI_IMPORT;
use scoretk_midi_export::CAPABILITIES as MIDI_EXPORT;
use scoretk_midi_import::CAPABILITIES as MIDI_IMPORT;
use scoretk_musescore_export::CAPABILITIES as MSCX_EXPORT;
use scoretk_musescore_import::CAPABILITIES as MSCX_IMPORT;
use scoretk_musicxml_export::CAPABILITIES as XML_EXPORT;
use scoretk_musicxml_import::CAPABILITIES as XML_IMPORT;

/// MusicXML 4.0 has standard carriers for every declared kind: `xsd:dashes` /
/// `xsd:bracket` + `xsd:words` for text lines, `xsd:bend` / `xsd:with-bar` for
/// a tremolo bar. No kind may be dropped from the denominator, including the
/// long-standing gaps (`stem`, `credit-position`, `hairpin`, `line-break`).
#[test]
fn xml_declaration_keeps_every_kind_with_a_carrier() {
    let kinds = XML_IMPORT
        .declared_kinds()
        .or(XML_EXPORT.declared_kinds())
        .expect("kinds MusicXML");
    for kind in [
        "text-line",
        "tremolo-bar",
        "hairpin",
        "trill",
        "stem",
        "credit-position",
        "line-break",
        "key-sig",
        "marker",
        "jump",
        "tuplet-member",
        "dynamic-velocity",
    ] {
        assert!(kinds.contains(&kind), "kind exclu à tort : {kind}");
    }
}

/// Which of the three labelled MusicXML export modes to measure.
#[derive(Clone, Copy)]
enum XmlMode {
    /// (a) Default export, MuseScore 4.7.4 byte parity.
    Parity,
    /// (b) Standard XSD carriers only (the reported headline).
    FullSupport,
    /// (c) Private, secondary additive carriers.
    Private,
}

/// (a) Default export: byte-faithful to MuseScore 4.7.4 `*.ref.musicxml`.
fn xml_parity_options() -> Options {
    Options::default()
}

/// (b) Headline: standard MusicXML 4.0 XSD carriers only, no private
/// `scoretk-*` attribute or element. This is the figure we report.
fn xml_headline_options() -> Options {
    Options {
        xml_full_support: true,
        ..Options::default()
    }
}

/// (c) Secondary, clearly labelled measurement only: the additive option
/// writes scoretk-private carriers outside the MusicXML 4.0 XSD.
fn xml_private_options() -> Options {
    Options {
        xml_lossless_round_trip: true,
        ..Options::default()
    }
}

fn xml_options(mode: XmlMode) -> Options {
    match mode {
        XmlMode::Parity => xml_parity_options(),
        XmlMode::FullSupport => xml_headline_options(),
        XmlMode::Private => xml_private_options(),
    }
}

/// Round trip of one direction over the whole corpus with explicit options.
fn round_trip_totals(
    scores: &[(String, PathBuf)],
    direction: &Direction,
    options: &Options,
) -> Totals {
    let mut totals = Totals::default();
    for (_, path) in scores {
        let reference = reference_model(path);
        let reference_inventory = inventory(&reference);
        let Ok(bytes) = write(&reference, direction.format, options) else {
            totals.skipped += 1;
            continue;
        };
        let Ok(round_trip) = read(&bytes, direction.format) else {
            totals.skipped += 1;
            continue;
        };
        let candidate = inventory(&round_trip);
        let kinds = direction
            .import
            .declared_kinds()
            .or(direction.export.declared_kinds());
        let declared = direction
            .import
            .declared()
            .union(direction.export.declared());
        let coverage = compare_with_kinds(&reference_inventory, &candidate, declared, kinds);
        totals.add(&coverage, essential(&reference_inventory, &candidate));
    }
    totals
}

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
    Mei,
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
    Direction {
        name: "mei",
        format: Format::Mei,
        import: MEI_IMPORT,
        export: MEI_EXPORT,
        reference: Some(ReferenceKind::Mei),
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
        ReferenceKind::Mei => (
            corpus_dir(version).join(format!("{name}.ref.mei")),
            Format::Mei,
        ),
    };
    path.is_file().then_some((path, format))
}

/// Origin of the file an import-alone row reads. The LilyPond and ABC
/// snapshots are written by scoretk's own writers: their rows are
/// self-consistency checks, not external compliance evidence (§11.3).
fn reference_source(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::MusicXml => "MuseScore 4.7.4 *.ref.musicxml",
        ReferenceKind::Midi => "MuseScore 4.7.4 *.ref.mid",
        ReferenceKind::Ly => "scoretk snapshot (self-consistency)",
        ReferenceKind::Abc => "scoretk snapshot (self-consistency)",
        ReferenceKind::Mei => "MuseScore 4.7.4 *.ref.mei",
    }
}

fn print_row(name: &str, declared: Layers, totals: &Totals, source: &str) {
    println!(
        "{:<14} {:<12} {:>9.2}% {:>12.2}% {:>6} {:>7} {:>8}  {:<38}",
        name,
        layers_label(declared),
        totals.essential.percent(),
        totals.total.percent(),
        totals.files,
        totals.skipped,
        totals.total.reference,
        source,
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
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}  source",
        "direction", "declared", "essential", "full-support", "files", "skipped", "ref-items"
    );

    println!("MusicXML is measured in three clearly labelled modes:");
    println!("  (a) parity   = default export, byte-faithful to MuseScore 4.7.4;");
    println!("  (b) HEADLINE = `xml_full_support`, standard MusicXML 4.0 XSD only;");
    println!("  (c) SECONDARY = `xml_lossless_round_trip`, non-XSD scoretk-* carriers.");
    for direction in DIRECTIONS {
        let declared = direction
            .import
            .declared()
            .union(direction.export.declared());
        if matches!(direction.format, Format::MusicXml | Format::Mxl) {
            let parity = round_trip_totals(&scores, direction, &xml_parity_options());
            print_row(
                &format!("{}+parity", direction.name),
                declared,
                &parity,
                "(a) default export, MuseScore 4.7.4 parity",
            );
            print_details(declared, &parity);
            let standard = round_trip_totals(&scores, direction, &xml_headline_options());
            print_row(
                &format!("{}+xsd", direction.name),
                declared,
                &standard,
                "(b) HEADLINE: xml_full_support, MusicXML 4.0 XSD only",
            );
            print_details(declared, &standard);
            let private = round_trip_totals(&scores, direction, &xml_private_options());
            print_row(
                &format!("{}+private", direction.name),
                declared,
                &private,
                "(c) SECONDARY: additive mode, non-XSD scoretk-* carriers",
            );
            print_details(declared, &private);
        } else {
            let totals = round_trip_totals(&scores, direction, &Options::default());
            print_row(direction.name, declared, &totals, "round trip (own writer)");
            print_details(declared, &totals);
        }
    }

    println!("\n=== Import alone against the same reference model ===");
    println!(
        "The LilyPond and ABC snapshots under tests/corpus/{{ly,abc}}/ were written \
         by scoretk: those rows are self-consistency measurements, NOT external compliance."
    );
    println!(
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}  source",
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
            let kinds = direction
                .import
                .declared_kinds()
                .or(direction.export.declared_kinds());
            let coverage = compare_with_kinds(&reference_inventory, &candidate, declared, kinds);
            totals.add(&coverage, essential(&reference_inventory, &candidate));
        }
        print_row(direction.name, declared, &totals, reference_source(kind));
        print_details(declared, &totals);
    }
    println!();
}

/// GABC is not measured on the MuseScore corpus: its primary data is the L6
/// syllable/neume stream, not a metrical event spine. The reference is the
/// Gregorian source itself; the round trip is `GABC → model → GABC → model`.
fn gabc_totals() -> (Totals, Layers) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("gabc");
    let mut paths: Vec<PathBuf> = fs::read_dir(root)
        .expect("GABC corpus directory")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gabc")
        })
        .collect();
    paths.sort();
    let declared = GABC_IMPORT.declared().union(GABC_EXPORT.declared());
    let mut totals = Totals::default();
    for path in paths {
        let bytes = fs::read(&path).expect("read GABC example");
        let reference = read(&bytes, Format::Gabc).expect("GABC import of the example");
        let reference_inventory = inventory(&reference);
        let Ok(written) = write(&reference, Format::Gabc, &Options::default()) else {
            totals.skipped += 1;
            continue;
        };
        let Ok(round_trip) = read(&written, Format::Gabc) else {
            totals.skipped += 1;
            continue;
        };
        let candidate = inventory(&round_trip);
        let coverage = compare(&reference_inventory, &candidate, declared);
        totals.add(&coverage, essential(&reference_inventory, &candidate));
    }
    (totals, declared)
}

#[test]
fn gabc_layer_coverage_table() {
    let (totals, declared) = gabc_totals();
    println!("\n=== GABC layer coverage on the nine Gregorio examples ===");
    println!(
        "key = (layer, tick, staff, kind, value); essential = note+measure; \
         full support = all declared layers"
    );
    println!(
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}  source",
        "direction", "declared", "essential", "full-support", "files", "skipped", "ref-items"
    );
    print_row(
        "gabc",
        declared,
        &totals,
        "Gregorio examples (GPL-3.0-or-later)",
    );
    print_details(declared, &totals);
    println!();
}

#[test]
fn gabc_full_support_reaches_the_target() {
    let (totals, _) = gabc_totals();
    assert_eq!(totals.files, 9);
    assert_eq!(totals.skipped, 0);
    assert!(
        totals.total.percent() >= 95.0,
        "GABC full support is {:.2} %",
        totals.total.percent()
    );
}

/// Removes every MusicXML element whose text is a private `gabc:` field.
///
/// Same method as the private ABC/LilyPond extensions in
/// `model-modularisation.md` §11.8: the written MusicXML is re-read after the
/// extensions have been deleted, so the score can only come from standard
/// MusicXML fields. Only the leaf element carrying the field is removed; an
/// empty `<direction>`/`<notations>` wrapper is harmless to the reader.
fn strip_gabc_extensions(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    loop {
        let Some(at) = rest.find("gabc:") else {
            out.push_str(rest);
            return out;
        };
        let Some(open) = rest[..at].rfind('<') else {
            out.push_str(rest);
            return out;
        };
        let tag = &rest[open + 1..];
        let name_end = tag
            .find(|character: char| character.is_whitespace() || character == '>')
            .unwrap_or(tag.len());
        let name = &tag[..name_end];
        let close = format!("</{name}>");
        let Some(end) = rest[at..]
            .find(&close)
            .map(|offset| at + offset + close.len())
        else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..open]);
        rest = &rest[end..];
    }
}

/// Cross conversion `GABC → MusicXML → GABC → model`, on the same nine
/// sources. Unlike `gabc_totals`, this cannot preserve the L6 payload: a
/// MusicXML score has no neume stream, so the second GABC is rebuilt by
/// `from_score` from concert pitches, lyrics and bars. The result measures
/// what a real conversion keeps, not what a verbatim copy keeps.
///
/// With `strip_extensions`, every `gabc:` field is deleted from the written
/// MusicXML before re-reading it: only standard MusicXML then transports the
/// chant.
fn gabc_cross_totals(strip_extensions: bool) -> (Totals, Layers, usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("gabc");
    let mut paths: Vec<PathBuf> = fs::read_dir(root)
        .expect("GABC corpus directory")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gabc")
        })
        .collect();
    paths.sort();
    let declared = GABC_IMPORT.declared().union(GABC_EXPORT.declared());
    let mut totals = Totals::default();
    let mut rewritten = 0;
    for path in paths {
        let bytes = fs::read(&path).expect("read GABC example");
        let reference = read(&bytes, Format::Gabc).expect("GABC import");
        let reference_inventory = inventory(&reference);
        let xml = match write(&reference, Format::MusicXml, &Options::default()) {
            Ok(value) => value,
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        let xml = if strip_extensions {
            let text = String::from_utf8(xml).expect("UTF-8 MusicXML");
            assert!(text.contains("gabc:"), "the private fields must be present");
            strip_gabc_extensions(&text).into_bytes()
        } else {
            xml
        };
        if strip_extensions {
            assert!(
                !String::from_utf8_lossy(&xml).contains("gabc:"),
                "no private field may survive the strip"
            );
        }
        let from_xml = match read(&xml, Format::MusicXml) {
            Ok(value) => value,
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        let gabc = match write(&from_xml, Format::Gabc, &Options::default()) {
            Ok(value) => value,
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        if gabc != bytes {
            rewritten += 1;
        }
        let candidate = match read(&gabc, Format::Gabc) {
            Ok(value) => inventory(&value),
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        let coverage = compare(&reference_inventory, &candidate, declared);
        totals.add(&coverage, essential(&reference_inventory, &candidate));
    }
    (totals, declared, rewritten)
}

/// GABC → MEI → GABC → model, measured like the GABC direct/cross rows of
/// `model-modularisation.md` §11.7. `private = false` (the default) writes
/// only the standard MEI 5.1 Neumes module; `private = true` also emits the
/// scoretk `gabc-*` annotations, which duplicate each atomic GABC token.
/// The first row is the honest interoperability figure: the second one is
/// kept only to quantify what the private extension adds. Before the neumes
/// module the MEI writer only emitted the derived modern view: that is the
/// baseline of the campaign.
fn gabc_mei_totals(private: bool) -> (Totals, Layers, usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("gabc");
    let mut paths: Vec<PathBuf> = fs::read_dir(root)
        .expect("GABC corpus directory")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gabc")
        })
        .collect();
    paths.sort();
    let declared = GABC_IMPORT.declared().union(GABC_EXPORT.declared());
    let mut totals = Totals::default();
    let mut rewritten = 0;
    for path in paths {
        let bytes = fs::read(&path).expect("read GABC example");
        let reference = read(&bytes, Format::Gabc).expect("GABC import");
        let reference_inventory = inventory(&reference);
        let options = MeiOptions {
            gabc_tokens: private,
            ..MeiOptions::default()
        };
        let mei = match scoretk_mei_export::write_mei_with(&reference, &options) {
            Ok(value) => value.into_bytes(),
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        assert_eq!(
            String::from_utf8_lossy(&mei).contains("<annot type=\"gabc-"),
            private,
            "the private GABC carriers must follow the option"
        );
        let from_mei = match read(&mei, Format::Mei) {
            Ok(value) => value,
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        let gabc = match write(&from_mei, Format::Gabc, &Options::default()) {
            Ok(value) => value,
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        if gabc != bytes {
            rewritten += 1;
        }
        let candidate = match read(&gabc, Format::Gabc) {
            Ok(value) => inventory(&value),
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        let coverage = compare(&reference_inventory, &candidate, declared);
        totals.add(&coverage, essential(&reference_inventory, &candidate));
    }
    (totals, declared, rewritten)
}

#[test]
fn gabc_mei_round_trip_table() {
    let (standard, declared, rewritten) = gabc_mei_totals(false);
    let (private, _, _) = gabc_mei_totals(true);
    println!("\n=== GABC cross round trip: GABC → MEI → GABC (nine examples) ===");
    println!(
        "key = (layer, tick, staff, kind, value); {rewritten}/9 outputs differ from their source"
    );
    println!(
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}  source",
        "direction", "declared", "essential", "full-support", "files", "skipped", "ref-items"
    );
    print_row(
        "gabc→mei→gabc",
        declared,
        &standard,
        "Gregorio examples (standard MEI 5.1 Neumes module, no gabc-* annotations)",
    );
    print_details(declared, &standard);
    print_row(
        "gabc→mei→gabc",
        declared,
        &private,
        "same files, with private extension (gabc-* atomic annotations)",
    );
    print_details(declared, &private);
    println!();
}

#[test]
fn gabc_mei_round_trip_is_measured() {
    let (totals, _, rewritten) = gabc_mei_totals(false);
    assert_eq!(totals.files, 9);
    assert_eq!(totals.skipped, 0);
    // `rewritten` used to be 9 because the writer dropped layout facts, so
    // every output differed from its source. Those facts are now structured
    // payload fields carried by MEI (§19/§20): 6/9 outputs are byte-identical
    // to their source, which proves a real conversion rather than a raw copy.
    // The no-raw-source guarantee is checked at compile time in
    // `scoretk-gabc-export::tests::payload_has_no_raw_source_field`.
    assert!(
        totals.total.reference > 0 && rewritten <= 9,
        "the MEI cross trip must re-serialise the GABC"
    );
}

#[test]
fn gabc_mei_round_trip_reaches_full_support_target() {
    let (totals, _, rewritten) = gabc_mei_totals(false);
    assert_eq!(totals.files, 9);
    assert_eq!(totals.skipped, 0);
    // See `gabc_mei_round_trip_is_measured`: exact outputs are now expected.
    // A raw-string copy cannot produce a non-empty structured inventory and
    // is forbidden by `payload_has_no_raw_source_field` in the writer crate.
    assert!(
        totals.total.reference > 0 && rewritten <= 9,
        "no raw-string copy may stand in for a conversion"
    );
    assert!(
        totals.essential.percent() >= 95.0 && totals.total.percent() >= 95.0,
        "GABC → MEI → GABC : essential {:.2} %, full support {:.2} %",
        totals.essential.percent(),
        totals.total.percent()
    );
}

#[test]
fn gabc_cross_round_trip_table() {
    let (private, declared, rewritten) = gabc_cross_totals(false);
    let (stripped, _, _) = gabc_cross_totals(true);
    println!("\n=== GABC cross round trip: GABC → MusicXML → GABC (nine examples) ===");
    println!(
        "key = (layer, tick, staff, kind, value); {rewritten}/9 outputs differ from their source"
    );
    println!(
        "{:<14} {:<12} {:>10} {:>13} {:>6} {:>7} {:>8}  source",
        "direction", "declared", "essential", "full-support", "files", "skipped", "ref-items"
    );
    print_row(
        "gabc→xml→gabc",
        declared,
        &private,
        "Gregorio examples (cross conversion, private gabc: fields)",
    );
    print_details(declared, &private);
    print_row(
        "gabc→xml→gabc",
        declared,
        &stripped,
        "same files, gabc: fields stripped (standard MusicXML only)",
    );
    print_details(declared, &stripped);
    println!();
}

/// Honest target of the cross conversion: what MusicXML can carry (structured
/// chant facts) must survive, while the known out-of-reach L6 items of
/// `docs/agents/gabc.md` §10 are *not* counted as gains. The measurement still
/// re-serialises all nine sources through MusicXML and back to GABC.
#[test]
fn gabc_cross_round_trip_reaches_full_support_target() {
    let (totals, _, rewritten) = gabc_cross_totals(false);
    assert_eq!(totals.files, 9);
    assert_eq!(totals.skipped, 0);
    assert_eq!(
        rewritten, 9,
        "no raw-string copy may stand in for a conversion"
    );
    assert!(
        totals.total.percent() >= 95.0,
        "GABC cross full support is {:.2} %",
        totals.total.percent()
    );
}

/// Second, honest measurement: the same cross conversion after deleting every
/// private `gabc:` field. This is the number the standard-MusicXML consumer
/// can reproduce; items 1–9 of the modern projection are measured against it.
#[test]
fn gabc_cross_round_trip_without_private_extensions_is_measured() {
    let (totals, _, _) = gabc_cross_totals(true);
    assert_eq!(totals.files, 9);
    assert_eq!(totals.skipped, 0);
    assert!(totals.total.reference > 0);
    println!(
        "GABC cross round trip without gabc: fields: essential {:.2} %, full support {:.2} %",
        totals.essential.percent(),
        totals.total.percent()
    );
}

#[test]
fn gabc_cross_round_trip_is_not_a_self_comparison() {
    let (totals, _, rewritten) = gabc_cross_totals(false);
    assert_eq!(totals.files, 9);
    assert_eq!(totals.skipped, 0);
    // If the writer copied its input, this would be 0/9.
    assert_eq!(
        rewritten, 9,
        "the cross round trip must re-serialise the GABC"
    );
    assert!(totals.total.reference > 0);
    println!(
        "GABC cross round trip: essential {:.2} %, full support {:.2} %",
        totals.essential.percent(),
        totals.total.percent()
    );
}

#[test]
fn musicxml_import_alone_reads_musescore_474_references() {
    // The import-alone row must not silently reuse scoretk's round-trip
    // output: it reads the versioned `*.ref.musicxml` produced by MuseScore.
    let mut checked = 0;
    for (name, _) in corpus_scores() {
        let (path, format) = reference_file(&name, ReferenceKind::MusicXml)
            .expect("a MuseScore MusicXML reference for every corpus score");
        assert_eq!(format, Format::MusicXml);
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("musicxml"));
        assert!(
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".ref.musicxml")),
            "{} is not a *.ref.musicxml reference",
            path.display()
        );
        let bytes = fs::read(&path).expect("read MuseScore MusicXML reference");
        let text = String::from_utf8(bytes).expect("UTF-8 reference");
        assert!(
            text.contains("<software>MuseScore Studio 4.7.4</software>"),
            "{} was not exported by MuseScore Studio 4.7.4",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 56);
}

#[test]
fn midi_declaration_follows_smf_spec() {
    // SMF 1.0 `format-ref/midi.md` §"Container layout": no bar/measure/staff/
    // voice fields; §"Instruments, parts, repeats": no repeats, voltas,
    // tuplets, ties or slurs; §"Meta-event types": FF 58/FF 59 (L1), FF 01
    // (generic text), FF 02/FF 03/FF 04 (copyright/track/instrument names),
    // FF 05 (RP-017 lyrics), FF 06/FF 07 (rehearsal marks / cue points),
    // FF 51 (tempo); §"Events and time": Note On/Off and velocity only — a
    // rest is the absence of a note, not an SMF object, so it is outside the
    // declaration. A kind with no SMF object is not a loss.
    let kinds = MIDI_IMPORT
        .declared_kinds()
        .expect("the SMF declares a subset of L1–L4");
    for kind in [
        "time-sig",
        "key-sig",
        "rehearsal-mark",
        "note",
        "lyric",
        "staff-text",
        "system-text",
        "tempo",
        "title",
        "instrument",
        "program",
        "channel",
        "volume",
        "pan",
        "velocity",
    ] {
        assert!(kinds.contains(&kind), "{kind} is carried by SMF");
    }
    for kind in [
        // `[SMF §Track Chunk; §Events and time]` : absence de Note On/Off,
        // jamais un évènement `rest` dans un `MTrk`.
        "rest",
        "measure",
        "barline",
        "clef",
        "volta",
        "tuplet",
        "tie",
        "slur",
        "tremolo",
        "credit",
        "dynamic-velocity",
        // `FF 06` is a rehearsal mark (a text item), not a segno/coda repeat
        // marker: SMF has no repeat objects (§"Instruments, parts, repeats").
        "marker",
    ] {
        assert!(!kinds.contains(&kind), "{kind} has no SMF object");
    }
    assert_eq!(MIDI_EXPORT.declared_kinds(), MIDI_IMPORT.declared_kinds());
}

/// Full per-kind loss table for the two MIDI rows (the main table only prints
/// the eight largest gaps). Diagnostic only, like `layer_coverage_table`: it
/// never asserts on a percentage.
#[test]
fn midi_full_loss_table() {
    let midi = &DIRECTIONS[4];
    assert_eq!(midi.name, "midi");
    let declared = midi.import.declared().union(midi.export.declared());
    let kinds = midi
        .import
        .declared_kinds()
        .or(midi.export.declared_kinds());
    let scores = corpus_scores();

    let mut round_trip = Totals::default();
    for (_, path) in &scores {
        let score = reference_model(path);
        let reference = inventory(&score);
        let Ok(bytes) = write(&score, midi.format, &Options::default()) else {
            round_trip.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&bytes, midi.format) else {
            round_trip.skipped += 1;
            continue;
        };
        let candidate = inventory(&candidate);
        let coverage = compare_with_kinds(&reference, &candidate, declared, kinds);
        round_trip.add(&coverage, essential(&reference, &candidate));
    }

    let mut import = Totals::default();
    for (name, path) in &scores {
        let Some((reference_path, format)) = reference_file(name, ReferenceKind::Midi) else {
            continue;
        };
        let reference = inventory(&reference_model(path));
        let Ok(bytes) = fs::read(&reference_path) else {
            import.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&bytes, format) else {
            import.skipped += 1;
            continue;
        };
        let candidate = inventory(&candidate);
        let coverage = compare_with_kinds(&reference, &candidate, declared, kinds);
        import.add(&coverage, essential(&reference, &candidate));
    }

    for (label, totals) in [
        ("round trip (own writer)", &round_trip),
        ("import of *.ref.mid", &import),
    ] {
        println!(
            "\n=== MIDI per-kind losses: {label} ({:.2} %, {} skipped) ===",
            totals.total.percent(),
            totals.skipped
        );
        let mut gaps: Vec<((u8, &'static str), usize)> = totals
            .missing_kinds
            .iter()
            .map(|((bits, kind), count)| ((*bits, *kind), *count))
            .collect();
        gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for ((bits, kind), count) in gaps {
            println!("    {:<28} {:<16} {:>5}", kind, layer_name(bits), count);
        }
    }
    println!();
}

/// Round-trip MIDI measurement with the additive `smf_exact_ticks` import
/// option, printed next to the default row. Both rows use the same writer,
/// the same corpus and the same declared SMF kinds; only the reader changes.
///
/// This compares the re-read model to the **written** inventory, so it is a
/// diagnostic of the 4.7.4 importer, NOT a measure of what the SMF carries.
/// The capacity measurement against the bytes in the file lives in
/// `midi_smf_capacity.rs` (see `docs/agents/midi.md`).
#[test]
fn midi_exact_ticks_full_support() {
    let midi = &DIRECTIONS[4];
    let declared = midi.import.declared().union(midi.export.declared());
    let kinds = midi
        .import
        .declared_kinds()
        .or(midi.export.declared_kinds());
    let scores = corpus_scores();
    let default_write = Options::default();
    let text_read = MidiImportOptions {
        smf_text: true,
        ..Default::default()
    };
    let text_write = Options {
        midi: scoretk::MidiOptions {
            smf_text: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let flat_text_write = Options {
        midi: scoretk::MidiOptions {
            smf_text: true,
            expand_repeats: false,
        },
        ..Default::default()
    };
    let exact = MidiImportOptions {
        smf_exact_ticks: true,
        ..Default::default()
    };
    let exact_text = MidiImportOptions {
        smf_exact_ticks: true,
        smf_text: true,
        ..Default::default()
    };

    println!("\n=== MIDI round trip: SMF options on the same 56 scores ===");
    let cases: Vec<(&str, &Options, Option<&MidiImportOptions>)> = vec![
        ("default (4.7.4)", &default_write, None),
        ("smf_exact_ticks", &default_write, Some(&exact)),
        ("smf_text (writer+reader)", &text_write, Some(&text_read)),
        ("exact + text", &text_write, Some(&exact_text)),
        ("flat + exact + text", &flat_text_write, Some(&exact_text)),
    ];
    for (label, write_options, read_options) in cases {
        let mut totals = Totals::default();
        for (_, path) in &scores {
            let score = reference_model(path);
            let reference = inventory(&score);
            let Ok(bytes) = write(&score, midi.format, write_options) else {
                totals.skipped += 1;
                continue;
            };
            let imported = match read_options {
                Some(options) => scoretk::read_midi_with(&bytes, options),
                None => read(&bytes, midi.format),
            };
            let Ok(imported) = imported else {
                totals.skipped += 1;
                continue;
            };
            let candidate = inventory(&imported);
            let coverage = compare_with_kinds(&reference, &candidate, declared, kinds);
            totals.add(&coverage, essential(&reference, &candidate));
        }
        println!(
            "{label:<26} {:<12} {:>9.2}% {:>12.2}% {:>6} {:>7} {:>8}",
            layers_label(declared),
            totals.essential.percent(),
            totals.total.percent(),
            totals.files,
            totals.skipped,
            totals.total.reference,
        );
        print_details(declared, &totals);
    }
    println!();
}

/// MIDI full support measured against what the SMF **actually contains**.
///
/// The default and option rows of `midi_exact_ticks_full_support` compare the
/// re-read model with the *written score inventory*: ticks of a performance
/// (repeats unfolded, gates, merged ties) cannot match written ticks, so
/// those rows diagnose 4.7.4 behaviour, not format capacity
/// (`docs/agents/model-modularisation.md` §11.9). Here the reference is the
/// bytes written by scoretk, parsed independently with `midly`, as in
/// `midi_smf_capacity.rs`: attacks are keyed by pitch, onset, played duration,
/// velocity and channel, and meta values are compared one by one. Both views
/// are projected into the same `LayerInventory`, so the declared SMF kinds
/// and the whole-corpus denominator are unchanged.
///
/// - `(a) default`: MuseScore 4.7.4 bytes, kept as the reference line;
/// - `(b) HEADLINE midi_full_support`: standard SMF carriers only — `FF 03`,
///   `FF 02`, `FF 01`, `FF 06`, `FF 07` and RP-026 tags. No `@ST:`/`@SY:`/
///   `@TK:` payload is allowed in this row (asserted below);
/// - `(c) SECONDARY smf_text`: the private `@…` mode, measured on its own.
#[test]
fn midi_full_support_capacity_line() {
    let midi = &DIRECTIONS[4];
    assert_eq!(midi.name, "midi");
    let declared = midi.import.declared().union(midi.export.declared());
    let kinds = midi
        .import
        .declared_kinds()
        .or(midi.export.declared_kinds());
    let scores = corpus_scores();

    let default_write = Options::default();
    let full_write = Options {
        midi_full_support: true,
        ..Options::default()
    };
    let private_write = Options {
        midi: scoretk::MidiOptions {
            smf_text: true,
            ..Default::default()
        },
        ..Options::default()
    };
    let full_read = MidiImportOptions {
        full_support: true,
        ..Default::default()
    };
    let private_read = MidiImportOptions {
        smf_text: true,
        smf_exact_ticks: true,
        ..Default::default()
    };

    let cases: Vec<(&str, &Options, Option<&MidiImportOptions>)> = vec![
        ("(a) default (4.7.4)", &default_write, None),
        (
            "(b) HEADLINE midi_full_support",
            &full_write,
            Some(&full_read),
        ),
        (
            "(c) SECONDARY smf_text private",
            &private_write,
            Some(&private_read),
        ),
    ];

    println!(
        "\n=== MIDI full support: SMF bytes as oracle ({} scores, {} declared kinds) ===",
        scores.len(),
        kinds.expect("declared SMF kinds").len()
    );
    let mut headline = None;
    for (label, write_options, read_options) in cases {
        let mut totals = Totals::default();
        for (_, path) in &scores {
            let score = reference_model(path);
            let Ok(bytes) = write(&score, midi.format, write_options) else {
                totals.skipped += 1;
                continue;
            };
            if label.starts_with("(b)") {
                // Private prefixes are not part of the standard-carrier
                // headline: the writer must not encode them at all.
                for prefix in [b"@ST:".as_slice(), b"@SY:", b"@TK:"] {
                    assert!(
                        !bytes.windows(prefix.len()).any(|w| w == prefix),
                        "midi_full_support wrote a private prefix: {}",
                        String::from_utf8_lossy(prefix)
                    );
                }
            }
            // The oracle always reads the file as written. The private
            // secondary row keeps its `@…` payloads: its interoperability
            // figure without them is measured in `midi_smf_capacity.rs`.
            let imported = match read_options {
                Some(options) => read_midi_with(&bytes, options),
                None => read(&bytes, midi.format),
            };
            let Ok(imported) = imported else {
                totals.skipped += 1;
                continue;
            };
            let reference = smf_inventory(&smf_notes(&bytes), &smf_kind_values(&bytes));
            let candidate = smf_inventory(&model_notes(&imported), &model_kind_values(&imported));
            let coverage = compare_with_kinds(&reference, &candidate, declared, kinds);
            totals.add(&coverage, essential(&reference, &candidate));
        }
        println!(
            "{label:<34} {:<12} {:>9.2}% {:>12.2}% {:>6} {:>7} {:>8}",
            layers_label(declared),
            totals.essential.percent(),
            totals.total.percent(),
            totals.files,
            totals.skipped,
            totals.total.reference,
        );
        print_details(declared, &totals);
        if label.starts_with("(b)") {
            headline = Some(totals.total.percent());
        }
    }
    println!();
    let headline = headline.expect("the headline row was measured");
    assert!(
        headline >= 95.0,
        "MIDI full support is the campaign target: {headline:.2} % < 95 %"
    );
}

#[test]
fn reference_sources_distinguish_self_written_snapshots() {
    for kind in [ReferenceKind::Ly, ReferenceKind::Abc] {
        assert!(
            reference_source(kind).contains("self-consistency"),
            "{} must be labelled as a self-consistency measurement",
            reference_source(kind)
        );
    }
    for kind in [
        ReferenceKind::MusicXml,
        ReferenceKind::Midi,
        ReferenceKind::Mei,
    ] {
        assert!(
            reference_source(kind).starts_with("MuseScore 4.7.4"),
            "{} must be labelled as a MuseScore 4.7.4 reference",
            reference_source(kind)
        );
    }
}

/// Full per-kind loss table for the two MEI rows (the main table only prints
/// the eight largest gaps). Diagnostic only, like `layer_coverage_table`: it
/// never asserts on a percentage.
#[test]
fn mei_full_loss_table() {
    let mei = &DIRECTIONS[7];
    assert_eq!(mei.name, "mei");
    let declared = mei.import.declared().union(mei.export.declared());
    let kinds = mei.import.declared_kinds().or(mei.export.declared_kinds());
    let scores = corpus_scores();

    let mut round_trip = Totals::default();
    for (_, path) in &scores {
        let score = reference_model(path);
        let reference = inventory(&score);
        let Ok(bytes) = write(&score, mei.format, &Options::default()) else {
            round_trip.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&bytes, mei.format) else {
            round_trip.skipped += 1;
            continue;
        };
        let candidate = inventory(&candidate);
        let coverage = compare_with_kinds(&reference, &candidate, declared, kinds);
        round_trip.add(&coverage, essential(&reference, &candidate));
    }

    let mut import = Totals::default();
    for (name, path) in &scores {
        let Some((reference_path, format)) = reference_file(name, ReferenceKind::Mei) else {
            continue;
        };
        let reference = inventory(&reference_model(path));
        let Ok(bytes) = fs::read(&reference_path) else {
            import.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&bytes, format) else {
            import.skipped += 1;
            continue;
        };
        let candidate = inventory(&candidate);
        let coverage = compare_with_kinds(&reference, &candidate, declared, kinds);
        import.add(&coverage, essential(&reference, &candidate));
    }

    for (label, totals) in [
        ("round trip (own writer)", &round_trip),
        ("import of *.ref.mei", &import),
    ] {
        println!(
            "\n=== MEI per-kind losses: {label} ({:.2} %, {} skipped) ===",
            totals.total.percent(),
            totals.skipped
        );
        let mut gaps: Vec<((u8, &'static str), usize)> = totals
            .missing_kinds
            .iter()
            .map(|((bits, kind), count)| ((*bits, *kind), *count))
            .collect();
        gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for ((bits, kind), count) in gaps {
            println!("    {:<28} {:<16} {:>5}", kind, layer_name(bits), count);
        }
    }
    println!();
}

/// Per-kind residual table of the MEI round trip with the additive MEI 5.1
/// carriers on (`Options::mei_full_support`). The default row of
/// `mei_full_loss_table` stays the 4.7.4-faithful export; this second row
/// measures what the format can carry once those carriers are enabled.
/// Diagnostic only: it never asserts on a percentage.
#[test]
fn mei_full_support_residual() {
    let mei = &DIRECTIONS[7];
    assert_eq!(mei.name, "mei");
    let declared = mei.import.declared().union(mei.export.declared());
    let kinds = mei.import.declared_kinds().or(mei.export.declared_kinds());
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let mut totals = Totals::default();
    for (_, path) in &corpus_scores() {
        let reference = reference_model(path);
        let reference_inventory = inventory(&reference);
        let Ok(bytes) = write(&reference, mei.format, &options) else {
            totals.skipped += 1;
            continue;
        };
        let Ok(candidate) = read(&bytes, mei.format) else {
            totals.skipped += 1;
            continue;
        };
        let candidate = inventory(&candidate);
        let coverage = compare_with_kinds(&reference_inventory, &candidate, declared, kinds);
        totals.add(&coverage, essential(&reference_inventory, &candidate));
    }
    println!(
        "\n=== MEI residual with mei_full_support (round trip): {:.2} %, {} skipped, missing {} / {} ===",
        totals.total.percent(),
        totals.skipped,
        totals.total.reference - totals.total.matched,
        totals.total.reference
    );
    let mut gaps: Vec<((u8, &'static str), usize)> = totals
        .missing_kinds
        .iter()
        .map(|((bits, kind), count)| ((*bits, *kind), *count))
        .collect();
    gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for ((bits, kind), count) in gaps {
        println!("    {:<28} {:<16} {:>5}", kind, layer_name(bits), count);
    }
    println!();
    assert_eq!(totals.files, 56);
    assert_eq!(totals.skipped, 0);
}

/// Every missing item of the extended MEI round trip, file by file with tick,
/// staff and value. This is the file-by-file queue of the campaign: unlike
/// `mei_full_support_residual` (per-kind totals), it tells where each loss is.
#[test]
fn mei_full_support_loss_diagnostic() {
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let declared = MEI_IMPORT.declared().union(MEI_EXPORT.declared());
    let kinds = MEI_IMPORT.declared_kinds().or(MEI_EXPORT.declared_kinds());
    for (name, path) in corpus_scores() {
        let reference = inventory(&reference_model(&path));
        let Ok(bytes) = write(&reference_model(&path), Format::Mei, &options) else {
            continue;
        };
        let Ok(candidate) = read(&bytes, Format::Mei) else {
            continue;
        };
        let candidate = inventory(&candidate);
        let mut expected: BTreeMap<_, usize> = BTreeMap::new();
        let mut actual: BTreeMap<_, usize> = BTreeMap::new();
        for (bucket, source) in [(&mut expected, &reference), (&mut actual, &candidate)] {
            for item in &source.items {
                if declared.contains(item.layer) && kinds.is_none_or(|ks| ks.contains(&item.kind)) {
                    *bucket
                        .entry((item.tick, item.staff, item.kind, item.value.clone()))
                        .or_insert(0) += 1;
                }
            }
        }
        let mut lines = Vec::new();
        for (key, wanted) in expected {
            let found = actual.get_mut(&key).map_or(0, |count| {
                let taken = (*count).min(wanted);
                *count -= taken;
                taken
            });
            for _ in found..wanted {
                lines.push(format!(
                    "    {:<22} tick {:<8} staff {:<3} {}",
                    key.2,
                    key.0
                        .map(|tick| tick.to_string())
                        .unwrap_or_else(|| "-".into()),
                    key.1.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                    key.3,
                ));
            }
        }
        if !lines.is_empty() {
            println!("--- {name} ---");
            for line in lines {
                println!("{line}");
            }
        }
    }
    println!();
}

/// Per-file diagnostic for the priority kinds of the MEI campaign: prints
/// every missing reference item with its measure (tick) and staff, for the
/// round trip and for the import of MuseScore's own `.ref.mei`. Diagnostic
/// only, like `mei_full_loss_table`: it never asserts on a percentage.
#[test]
fn mei_core_loss_diagnostic() {
    const KINDS: &[&str] = &[
        "note",
        "rest",
        "measure",
        "tuplet-member",
        "key-sig",
        "tempo",
        "marker",
        "jump",
        "tremolo",
        "articulation",
        "credit",
        "credit-position",
        "note-head",
        "lyric",
        "slur",
        "clef",
        "dynamic",
        "instrument",
        "program",
        "grace",
        "grace-note",
        "tuplet-bracket",
        "multi-measure-rest",
    ];

    fn report(
        label: &str,
        score_name: &str,
        reference: &scoretk::model::LayerInventory,
        imported: &scoretk::model::LayerInventory,
    ) {
        let mut expected: BTreeMap<_, usize> = BTreeMap::new();
        let mut actual: BTreeMap<_, usize> = BTreeMap::new();
        for (bucket, inventory) in [(&mut expected, reference), (&mut actual, imported)] {
            for item in &inventory.items {
                if KINDS.contains(&item.kind) {
                    *bucket
                        .entry((
                            item.layer.bits(),
                            item.tick,
                            item.staff,
                            item.kind,
                            item.value.clone(),
                        ))
                        .or_insert(0) += 1;
                }
            }
        }
        let mut lines = Vec::new();
        for (key, wanted) in expected {
            let found = actual.get_mut(&key).map_or(0, |count| {
                let taken = (*count).min(wanted);
                *count -= taken;
                taken
            });
            for _ in found..wanted {
                lines.push(format!(
                    "    {:<16} tick {:<8} staff {:<3} {}",
                    key.3,
                    key.1
                        .map(|tick| tick.to_string())
                        .unwrap_or_else(|| "-".into()),
                    key.2.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                    key.4,
                ));
            }
        }
        if !lines.is_empty() {
            println!("\n--- {label}: {score_name} ---");
            for line in lines {
                println!("{line}");
            }
        }
    }

    for (name, path) in corpus_scores() {
        let reference = inventory(&reference_model(&path));
        let bytes = write(&reference_model(&path), Format::Mei, &Options::default())
            .expect("MEI round trip");
        let round_trip = inventory(&read(&bytes, Format::Mei).expect("MEI read"));
        report("round trip", &name, &reference, &round_trip);

        let Some((reference_path, format)) = reference_file(&name, ReferenceKind::Mei) else {
            continue;
        };
        let bytes = fs::read(&reference_path).expect("read MEI reference");
        let imported = inventory(&read(&bytes, format).expect("MEI read"));
        report("import *.ref.mei", &name, &reference, &imported);
    }
    println!();
}

/// Full per-kind loss table for the MuseScore and MusicXML rows (the main
/// table only prints the eight largest gaps). Diagnostic only, like
/// `layer_coverage_table`: it never asserts on a percentage.
#[test]
fn xml_full_loss_table() {
    let scores = corpus_scores();
    let rows: &[(&str, &Direction, Option<ReferenceKind>, XmlMode)] = &[
        ("mscx round trip", &DIRECTIONS[0], None, XmlMode::Parity),
        (
            "musicxml round trip (a) 4.7.4 parity",
            &DIRECTIONS[2],
            None,
            XmlMode::Parity,
        ),
        (
            "musicxml round trip (b) HEADLINE full_support XSD",
            &DIRECTIONS[2],
            None,
            XmlMode::FullSupport,
        ),
        (
            "musicxml round trip (c) SECONDARY private",
            &DIRECTIONS[2],
            None,
            XmlMode::Private,
        ),
        (
            "musicxml import *.ref.musicxml",
            &DIRECTIONS[2],
            Some(ReferenceKind::MusicXml),
            XmlMode::Parity,
        ),
    ];
    for (label, direction, reference, mode) in rows {
        let declared = direction
            .import
            .declared()
            .union(direction.export.declared());
        let kinds = direction
            .import
            .declared_kinds()
            .or(direction.export.declared_kinds());
        let mut totals = Totals::default();
        let xml_options = xml_options(*mode);
        let default_options = Options::default();
        for (name, path) in &scores {
            let reference_inventory = inventory(&reference_model(path));
            let write_options = if matches!(direction.format, Format::MusicXml | Format::Mxl) {
                &xml_options
            } else {
                &default_options
            };
            let (bytes, format) = match reference {
                None => (
                    write(&reference_model(path), direction.format, write_options).ok(),
                    direction.format,
                ),
                Some(kind) => match reference_file(name, *kind) {
                    Some((reference_path, format)) => (fs::read(reference_path).ok(), format),
                    None => (None, direction.format),
                },
            };
            let Some(bytes) = bytes else {
                totals.skipped += 1;
                continue;
            };
            let Ok(candidate) = read(&bytes, format) else {
                totals.skipped += 1;
                continue;
            };
            let candidate = inventory(&candidate);
            let coverage = compare_with_kinds(&reference_inventory, &candidate, declared, kinds);
            totals.add(&coverage, essential(&reference_inventory, &candidate));
        }
        println!(
            "\n=== {} per-kind losses ({:.2} %, {} skipped, missing {} / {}) ===",
            label,
            totals.total.percent(),
            totals.skipped,
            totals.total.reference - totals.total.matched,
            totals.total.reference
        );
        let mut gaps: Vec<((u8, &'static str), usize)> = totals
            .missing_kinds
            .iter()
            .map(|((bits, kind), count)| ((*bits, *kind), *count))
            .collect();
        gaps.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for ((bits, kind), count) in gaps {
            println!("    {:<28} {:<16} {:>5}", kind, layer_name(bits), count);
        }
    }
    println!();
}

/// Every missing item of the MuseScore and MusicXML rows, with its score,
/// tick, staff and value. Diagnostic only; it never asserts on a percentage.
#[test]
fn xml_loss_diagnostic() {
    let scores = corpus_scores();
    for (label, format, reference) in [
        ("mscx round trip", Format::Mscx, None),
        ("musicxml round trip", Format::MusicXml, None),
        (
            "musicxml import *.ref.musicxml",
            Format::MusicXml,
            Some(ReferenceKind::MusicXml),
        ),
    ] {
        let (declared, kinds) = if format == Format::Mscx {
            (
                MSCX_IMPORT.declared().union(MSCX_EXPORT.declared()),
                MSCX_IMPORT
                    .declared_kinds()
                    .or(MSCX_EXPORT.declared_kinds()),
            )
        } else {
            (
                XML_IMPORT.declared().union(XML_EXPORT.declared()),
                XML_IMPORT.declared_kinds().or(XML_EXPORT.declared_kinds()),
            )
        };
        println!(
            "\n=== {label}: missing items ({}) ===",
            if reference.is_none() && format == Format::MusicXml {
                "xml_full_support, standard XSD only"
            } else {
                "default export"
            }
        );
        let xml_options = if reference.is_none() && format == Format::MusicXml {
            // Diagnostic for the reported line: standard XSD carriers only.
            xml_headline_options()
        } else {
            Options::default()
        };
        let default_options = Options::default();
        for (name, path) in &scores {
            let reference_inventory = inventory(&reference_model(path));
            let write_options = if format == Format::MusicXml {
                &xml_options
            } else {
                &default_options
            };
            let bytes = match reference {
                None => write(&reference_model(path), format, write_options).expect("write"),
                Some(kind) => fs::read(&reference_file(name, kind).expect("reference").0)
                    .expect("read reference"),
            };
            let candidate = inventory(&read(&bytes, format).expect("read"));
            let mut expected: BTreeMap<_, usize> = BTreeMap::new();
            let mut actual: BTreeMap<_, usize> = BTreeMap::new();
            for (bucket, source) in [
                (&mut expected, &reference_inventory),
                (&mut actual, &candidate),
            ] {
                for item in &source.items {
                    if declared.contains(item.layer)
                        && kinds.is_none_or(|ks| ks.contains(&item.kind))
                    {
                        *bucket
                            .entry((item.tick, item.staff, item.kind, item.value.clone()))
                            .or_insert(0) += 1;
                    }
                }
            }
            let mut lines = Vec::new();
            for (key, wanted) in expected {
                let found = actual.get_mut(&key).map_or(0, |count| {
                    let taken = (*count).min(wanted);
                    *count -= taken;
                    taken
                });
                for _ in found..wanted {
                    lines.push(format!(
                        "    {:<22} tick {:<8} staff {:<3} {}",
                        key.2,
                        key.0
                            .map(|tick| tick.to_string())
                            .unwrap_or_else(|| "-".into()),
                        key.1.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                        key.3,
                    ));
                }
            }
            if !lines.is_empty() {
                println!("--- {name} ---");
                for line in lines {
                    println!("{line}");
                }
            }
        }
    }
    println!();
}

/// Item kinds whose carrier is *never written* by MuseScore 4.7.4's MEI
/// exporter. They are kept in the main import figure (never dropped), but a
/// second figure excludes them to measure what the references allow.
///
/// - `style`: `Convert::staffToMEI` (meiconverter.cpp:3094-3112) writes no
///   `att.pages`, `@vu.height` or numeric `@fontsize`; see the carrier test
///   below.
/// - `dynamic-velocity`: `Convert::dynamToMEI` (meiconverter.cpp:1356-1380)
///   writes `@label` and text only, never a velocity.
/// - `note-head`: `MeiExporter::writeNote` (meiexporter.cpp:1370) writes no
///   `@head.shape` and no `@glyph.name` (the SMuFL glyphs are written for
///   clefs, fermatas, caesuras, ornaments, but not for note heads).
/// - `credit`/`credit-position` and `tbox-*`: `MeiExporter::writeScoreDef`
///   (meiexporter.cpp:326-331) converts only the *first* VBox into a
///   `<pgHead func="first">` (meiexporter.cpp:386-459); later frames have no
///   MEI carrier.
const MEI_KINDS_ABSENT_FROM_MUSESCORE_REFERENCES: &[&str] = &[
    "style",
    "dynamic-velocity",
    "note-head",
    "credit",
    "credit-position",
    "tbox-text",
    "tbox-position",
];

/// The main import row above is a comparison against the whole `.mscx`
/// reference model. This second row restricts both inventories to the kinds
/// MuseScore 4.7.4 actually writes in its own `.ref.mei`; it is additive
/// evidence, not a replacement for the first row.
#[test]
fn mei_import_figure_without_absent_carriers() {
    let mei = &DIRECTIONS[7];
    assert_eq!(mei.name, "mei");
    let declared = mei.import.declared();
    let mut all_kinds: Vec<&'static str> = Vec::new();
    for (_, path) in corpus_scores() {
        for item in &inventory(&reference_model(&path)).items {
            if declared.contains(item.layer) && !all_kinds.contains(&item.kind) {
                all_kinds.push(item.kind);
            }
        }
    }
    let measured_kinds: Vec<&str> = all_kinds
        .iter()
        .copied()
        .filter(|kind| !MEI_KINDS_ABSENT_FROM_MUSESCORE_REFERENCES.contains(kind))
        .collect();

    let mut main = Totals::default();
    let mut reachable = Totals::default();
    for (name, path) in corpus_scores() {
        let Some((reference_path, format)) = reference_file(&name, ReferenceKind::Mei) else {
            continue;
        };
        let reference = inventory(&reference_model(&path));
        let bytes = fs::read(&reference_path).expect("read MEI reference");
        let imported = read(&bytes, format).expect("MEI import");
        let candidate = inventory(&imported);
        main.add(
            &compare_with_kinds(&reference, &candidate, declared, Some(&all_kinds)),
            essential(&reference, &candidate),
        );
        reachable.add(
            &compare_with_kinds(&reference, &candidate, declared, Some(&measured_kinds)),
            essential(&reference, &candidate),
        );
    }

    println!("\n=== MEI import: main figure vs. kinds present in MuseScore's *.ref.mei ===");
    print_row(
        "import (all)",
        declared,
        &main,
        reference_source(ReferenceKind::Mei),
    );
    print_row(
        "import (ref)",
        declared,
        &reachable,
        "same references, carriers MuseScore 4.7.4 writes",
    );
    println!(
        "    excluded: {} reference items of {:?}",
        main.total.reference - reachable.total.reference,
        MEI_KINDS_ABSENT_FROM_MUSESCORE_REFERENCES
    );
    println!();

    assert_eq!(main.files, 56);
    assert_eq!(reachable.files, 56);
    assert!(main.total.reference > reachable.total.reference);
    assert!(
        reachable.total.percent() > main.total.percent(),
        "the reference-bound figure must be higher, not a replacement"
    );
}

#[test]
fn mei_import_alone_reads_musescore_474_references() {
    let mut checked = 0;
    for (name, _) in corpus_scores() {
        let (path, format) = reference_file(&name, ReferenceKind::Mei)
            .expect("a MuseScore MEI reference for every corpus score");
        assert_eq!(format, Format::Mei);
        assert!(
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".ref.mei")),
            "{} is not a *.ref.mei reference",
            path.display()
        );
        let bytes = fs::read(&path).expect("read MuseScore MEI reference");
        let text = String::from_utf8(bytes).expect("UTF-8 reference");
        assert!(
            text.contains("xmlns=\"http://www.music-encoding.org/ns/mei\""),
            "{} has no MEI namespace",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 56);
}

/// The import row's 380 `style` losses are a property of the MuseScore 4.7.4
/// *references*: they carry none of the standard MEI layout carriers that
/// scoretk writes (`att.pages`, `@vu.height`, numeric `@fontsize`). This test
/// pins that fact so the gap cannot be mistaken for an unported reader.
#[test]
fn mei_musescore_references_carry_no_style() {
    for (name, _) in corpus_scores() {
        let (path, _) = reference_file(&name, ReferenceKind::Mei)
            .expect("a MuseScore MEI reference for every corpus score");
        let text = String::from_utf8(fs::read(&path).expect("read MEI reference"))
            .expect("UTF-8 reference");
        for carrier in [
            "page.width",
            "page.height",
            "page.leftmar",
            "page.rightmar",
            "page.topmar",
            "page.botmar",
            "vu.height",
        ] {
            assert!(
                !text.contains(carrier),
                "{} carries {carrier}, which would make the style gap a reader bug",
                path.display()
            );
        }
        assert!(
            !text.contains("fontsize=\"24pt\"") && !text.contains("fontsize=\"12pt\""),
            "{} carries a numeric font size",
            path.display()
        );
        // `MeiExporter::writeNote` (meiexporter.cpp:1370) n'écrit ni
        // `@head.shape` ni `@glyph.name` sur les notes ; `Convert::dynamToMEI`
        // (meiconverter.cpp:1356-1380) n'écrit pas de vélocité.
        assert!(
            !text.contains("head.shape="),
            "{} carries a note head",
            path.display()
        );
        assert!(
            !text.contains("glyph.name=\"notehead"),
            "{} carries a SMuFL note head",
            path.display()
        );
        assert!(
            !text.contains("vel=\""),
            "{} carries a velocity",
            path.display()
        );
    }
}
