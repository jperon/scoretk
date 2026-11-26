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

//! GABC → every other format → model, measured against the derived modern view.
//!
//! The reference is the score read from the nine versioned Gregorio examples:
//! its L0/L1 spine is the **derived projection** documented in
//! `docs/agents/gabc.md` §7 (one quarter per sung letter, a punctum mora
//! doubles the written figure, an episema is a tenuto played ×1.5, a quilisma
//! is an inverted mordent with progressive holds, one measure per separation
//! bar). The ten chant facts compared here are `pitch`, `duration`, `lyric`,
//! `bar`, `clef`, `accidental`, `breath`, `episema`, `quilisma`, `title`.
//!
//! A format's event timeline is part of its specification: MusicXML
//! `<duration>`, MuseScore `actualTicks()` and SMF delta times advance by
//! **played** time, while LilyPond and ABC note lengths are **written** time.
//! The reference is projected onto that timeline before comparison, so a tick
//! shift caused by the target's own time model is not reported as a lost note
//! (`docs/agents/gabc.md` §14). The `duration` item likewise contains exactly
//! the components the destination can carry (written figure, played ticks, or
//! both). An item is keyed by `(kind, tick, value)` and compared as a multiset.
//!
//! **Essential** = `pitch` + `bar` (the `note` + `measure` analogue of §11).
//! **Full support** = the kinds the destination's specification can carry;
//! the declaration and its citation live in `DESTINATIONS` and are documented
//! in `docs/agents/gabc.md` §14.
//!
//! This test never fails on a percentage: it prints a measurement table and
//! panics only on a broken setup. Run with `--nocapture`.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use scoretk::model::{
    AccidentalType, ArticulationType, Fraction, OrnamentType, Score, SegmentItem,
};
use scoretk::{read, write, Format, Options};

/// The ten chant facts measured here, in report order.
const KINDS: &[&str] = &[
    "pitch",
    "duration",
    "lyric",
    "bar",
    "clef",
    "accidental",
    "breath",
    "episema",
    "quilisma",
    "title",
];

/// Timeline used by a destination's event model.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Timeline {
    /// Written figure grid (`format-ref/lilypond.md` §Durations,
    /// `format-ref/abc.md` §4.3): note lengths advance the cursor.
    Written,
    /// Played-time grid (`format-ref/musicxml.md` §Units and time encoding,
    /// `format-ref/musescore.md` §Unit system, `format-ref/midi.md` §Events).
    Played,
    /// The ticks the format's reader actually produced. Used for the candidate:
    /// readers place events on their own timeline, so re-projecting them would
    /// count the same played durations twice.
    AsIs,
}

/// Components of `ChordRest::duration`/`ticks` a destination can carry.
#[derive(Clone, Copy)]
enum DurationCarrier {
    Written,
    Played,
    Both,
}

#[derive(Clone, Copy)]
struct View {
    timeline: Timeline,
    duration: DurationCarrier,
    /// Le rendu SMF insère une pause pour chaque barre de séparation
    /// (`SegmentItem::Breath::pause_ms`, `docs/agents/timemap.md`) : la
    /// projection de référence doit la cumuler pour retrouver les onsets.
    pauses: bool,
    /// Le SMF n'a pas de tick écrit : comparez les faits sans ancrage.
    no_ticks: bool,
    /// RP-017 porte une syllabe par méta-évènement, sans marqueur
    /// begin/middle/end : seul le texte est comparable.
    lyric_text_only: bool,
}

const WRITTEN: View = View {
    timeline: Timeline::Written,
    duration: DurationCarrier::Written,
    pauses: false,
    no_ticks: false,
    lyric_text_only: false,
};
const PLAYED: View = View {
    timeline: Timeline::Played,
    duration: DurationCarrier::Both,
    pauses: false,
    no_ticks: false,
    lyric_text_only: false,
};
const MIDI_VIEW: View = View {
    timeline: Timeline::Played,
    duration: DurationCarrier::Played,
    pauses: false,
    no_ticks: true,
    lyric_text_only: true,
};

/// One destination, the kinds its **specification** can carry, and the view
/// used to project the reference onto its timeline.
///
/// The declaration is about the format, not about today's writer: a declared
/// kind that is absent after a round trip is a fixable transport gap; a kind
/// absent from the declaration is an inherent loss, cited in `spec` and in
/// `docs/agents/gabc.md` §14.
struct Destination {
    name: &'static str,
    format: Format,
    view: View,
    declared: &'static [&'static str],
    spec: &'static str,
}

const ALL_KINDS: &[&str] = KINDS;
/// Kinds the SMF specification declares a carrier for (§`Destination::declared`):
/// Note On/Off for pitch and duration, `FF 05` for lyric text and `FF 03`/`FF 01`
/// for names. "Declared" is about the format, not about today's writer: the
/// round-trip misses below are transport/comparator gaps, not inherent losses.
const DECLARED_BY_SMF: &[&str] = &["pitch", "duration", "lyric", "title"];

const DESTINATIONS: &[Destination] = &[
    Destination {
        name: "musicxml",
        format: Format::MusicXml,
        view: WRITTEN,
        declared: ALL_KINDS,
        spec: "format-ref/musicxml.md §Document shape, §Units and time encoding, §Pitch accidentals, §Clef, §Notes, §Repeats directions lyrics; xsd:breath-mark, xsd:notehead/@smufl, xsd:other-notation/@smufl",
    },
    Destination {
        name: "mxl",
        format: Format::Mxl,
        view: WRITTEN,
        declared: ALL_KINDS,
        spec: "format-ref/musicxml.md §Document shape (.mxl container), same carriers as musicxml",
    },
    Destination {
        name: "mscx",
        format: Format::Mscx,
        view: WRITTEN,
        declared: ALL_KINDS,
        spec: "format-ref/musescore.md §Section index (§5 bars, §7 figures/tpc, §10 clefs, §13 lyrics, §18 texts); mscx-format.md §3, §7, §10",
    },
    Destination {
        name: "mscz",
        format: Format::Mscz,
        view: WRITTEN,
        declared: ALL_KINDS,
        spec: "format-ref/musescore.md §`.mscz` container, same carriers as mscx",
    },
    Destination {
        name: "lilypond",
        format: Format::Ly,
        view: WRITTEN,
        declared: ALL_KINDS,
        spec: "format-ref/lilypond.md §Durations, §Pitch clef accidentals, §Articulations ornaments (\\breathe, -_, \\prall), §Repeats and lyrics, §File structure",
    },
    Destination {
        name: "abc",
        format: Format::Abc,
        view: WRITTEN,
        declared: ALL_KINDS,
        spec: "format-ref/abc.md §3.1 headers, §4.3 durations, §4.2 accidentals, §4.6 clef, §4.8 barlines, §4.14 decorations (!shortphrase!, !tenuto!, P), §5 lyrics",
    },
    Destination {
        name: "midi",
        format: Format::Midi,
        view: MIDI_VIEW,
        declared: DECLARED_BY_SMF,
        spec: "format-ref/midi.md §Events and time, §Meta-event types (FF 03 name, FF 05 lyrics), §Instruments parts repeats; no clef/bar/accidental/ornament object",
    },
];

type Key = (&'static str, Option<Fraction>, String);
type Inventory = BTreeMap<Key, usize>;

fn push(inventory: &mut Inventory, kind: &'static str, tick: Option<Fraction>, value: String) {
    *inventory.entry((kind, tick, value)).or_insert(0) += 1;
}

fn played(chord_rest: &scoretk::model::ChordRest) -> Fraction {
    chord_rest
        .ticks
        .unwrap_or_else(|| chord_rest.duration.ticks())
}

fn duration_value(
    duration: &scoretk::model::Duration,
    played: Fraction,
    carrier: DurationCarrier,
) -> String {
    let written = format!("{:?}:{}", duration.kind, duration.dots);
    match carrier {
        DurationCarrier::Written => written,
        DurationCarrier::Played => played.to_string(),
        DurationCarrier::Both => format!("{written}:{played}"),
    }
}

/// The derived modern view, projected onto one destination's timeline and
/// reduced to the ten facts a format may carry.
fn chant_inventory(score: &Score, view: View) -> Inventory {
    let mut inventory = Inventory::new();
    let mut pause_offset = Fraction::ZERO;
    for measure in &score.measures {
        let mut cursor = if view.timeline == Timeline::Played {
            measure.start_tick + pause_offset
        } else {
            measure.start_tick
        };
        for segment in &measure.segments {
            let tick = if view.no_ticks {
                None
            } else {
                match view.timeline {
                    Timeline::Written | Timeline::AsIs => Some(segment.tick),
                    Timeline::Played => Some(cursor),
                }
            };
            for (_, element) in segment.iter_elements() {
                match element {
                    SegmentItem::Clef(clef) => {
                        push(&mut inventory, "clef", tick, format!("{:?}", clef.concert));
                    }
                    SegmentItem::Breath(breath) => {
                        // Seule la virgule est un signe de respiration
                        // (`BarKind::has_breath`, `Gabc.tex:812-820`). Les
                        // césures des autres barres sont des pauses d'exécution
                        // de la projection (timeline MIDI), pas un fait
                        // grégorien : elles ne font pas partie des dix faits.
                        // MusicXML ancre la virgule à la dernière note
                        // chantée (XSD `note`), MuseScore à un segment dédié :
                        // seul le signe est comparable, pas son ancrage.
                        if breath.kind == scoretk::model::BreathType::Comma {
                            push(&mut inventory, "breath", None, "comma".to_owned());
                        }
                    }
                    SegmentItem::ChordRest(chord_rest) => {
                        let performed = played(chord_rest);
                        // Les silences de remplissage ne sont pas un fait
                        // grégorien : GABC n'a pas de token de silence.
                        if chord_rest.chord_ref().is_some() {
                            push(
                                &mut inventory,
                                "duration",
                                tick,
                                duration_value(&chord_rest.duration, performed, view.duration),
                            );
                        }
                        if let scoretk::model::ChordRestKind::Chord(chord) = &chord_rest.kind {
                            for id in &chord.notes {
                                let note = &score.notes[id.index()];
                                push(&mut inventory, "pitch", tick, note.pitch.to_string());
                                if let Some(accidental) = note.accidental {
                                    if accidental.kind != AccidentalType::None {
                                        push(
                                            &mut inventory,
                                            "accidental",
                                            tick,
                                            format!(
                                                "{:?}:{}:{}",
                                                accidental.kind,
                                                accidental.parentheses,
                                                accidental.user
                                            ),
                                        );
                                    }
                                }
                            }
                            for articulation in &chord.articulations {
                                if articulation.kind == ArticulationType::Tenuto {
                                    push(&mut inventory, "episema", tick, "tenuto".to_owned());
                                }
                            }
                            for ornament in &chord.ornaments {
                                if ornament.kind == OrnamentType::InvertedMordent {
                                    push(
                                        &mut inventory,
                                        "quilisma",
                                        tick,
                                        "inverted-mordent".to_owned(),
                                    );
                                }
                            }
                        }
                        for lyric in &chord_rest.lyrics {
                            if lyric.verse == 0 {
                                push(
                                    &mut inventory,
                                    "lyric",
                                    tick,
                                    if view.lyric_text_only {
                                        lyric.text.clone()
                                    } else {
                                        format!("{:?}:{}", lyric.syllabic, lyric.text)
                                    },
                                );
                            }
                        }
                        if matches!(view.timeline, Timeline::Played) {
                            cursor += performed;
                        }
                    }
                    _ => {}
                }
            }
        }
        if view.pauses {
            for segment in &measure.segments {
                if let Some(scoretk::model::SegmentItem::Breath(breath)) = segment.element(
                    scoretk::model::Track::new(scoretk::model::StaffId::from_index(0), 0),
                ) {
                    if let Some(milliseconds) = breath.pause_ms {
                        // 120 BPM : une noire vaut 500 ms (`format-ref/midi.md`
                        // §Meta-event types, `FF 51` = 500000 µs).
                        let pause = Fraction::new(i64::from(milliseconds), 2000);
                        cursor += pause;
                        pause_offset += pause;
                    }
                }
            }
        }
        if measure.end_bar_line != scoretk::model::BarLineType::Normal {
            let tick = if view.no_ticks {
                None
            } else {
                match view.timeline {
                    Timeline::Written | Timeline::AsIs => Some(measure.end_tick()),
                    Timeline::Played => Some(cursor),
                }
            };
            push(
                &mut inventory,
                "bar",
                tick,
                format!("{:?}", measure.end_bar_line),
            );
        }
    }
    for (name, value) in [
        ("work_title", &score.metadata.work_title),
        ("movement_title", &score.metadata.movement_title),
        ("composer", &score.metadata.composer),
        ("lyricist", &score.metadata.lyricist),
    ] {
        if !value.is_empty() {
            push(&mut inventory, "title", None, format!("{name}:{value}"));
        }
    }
    inventory
}

#[derive(Default)]
struct Counts {
    reference: usize,
    matched: usize,
    extra: usize,
    missing_by_kind: BTreeMap<&'static str, usize>,
}

fn compare(reference: &Inventory, candidate: &Inventory, declared: &[&str]) -> Counts {
    let mut counts = Counts::default();
    let mut actual = candidate.clone();
    for ((kind, tick, value), wanted) in reference {
        if !declared.contains(kind) {
            continue;
        }
        counts.reference += wanted;
        let key = (*kind, *tick, value.clone());
        let found = actual.get_mut(&key).map_or(0, |have| {
            let taken = (*have).min(*wanted);
            *have -= taken;
            taken
        });
        counts.matched += found;
        if *wanted > found {
            let at = counts.missing_by_kind.entry(*kind).or_insert(0);
            *at += wanted - found;
        }
    }
    for ((kind, _, _), left) in &actual {
        if declared.contains(kind) {
            counts.extra += left;
        }
    }
    counts
}

impl Counts {
    fn percent(&self) -> f64 {
        if self.reference == 0 {
            100.0
        } else {
            self.matched as f64 * 100.0 / self.reference as f64
        }
    }
}

fn corpus() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
    paths
}

/// Removes every MusicXML element carrying a private `gabc:` field, as in
/// `layer_coverage.rs`: the score can then only come from standard MusicXML.
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

/// Removes the private fields from a written MusicXML document or `.mxl`
/// container. The container is rebuilt entry by entry so the reader sees the
/// same `<rootfile>` and only standard MusicXML.
fn standardise(bytes: Vec<u8>, format: Format) -> Vec<u8> {
    match format {
        Format::MusicXml => {
            strip_gabc_extensions(&String::from_utf8(bytes).expect("UTF-8 XML")).into_bytes()
        }
        Format::Mxl => {
            let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("read .mxl");
            let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
            let mut entries = Vec::new();
            for name in names {
                let mut data = Vec::new();
                archive
                    .by_name(&name)
                    .expect("zip entry")
                    .read_to_end(&mut data)
                    .expect("read zip entry");
                if name.ends_with(".xml") || name.ends_with(".musicxml") {
                    data = strip_gabc_extensions(&String::from_utf8(data).expect("UTF-8 XML"))
                        .into_bytes();
                }
                entries.push((name, data));
            }
            let mut output = Vec::new();
            {
                let mut zip = ZipWriter::new(Cursor::new(&mut output));
                for (name, data) in entries {
                    zip.start_file(
                        name.as_str(),
                        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
                    )
                    .expect("start zip entry");
                    zip.write_all(&data).expect("write zip entry");
                }
                zip.finish().expect("finish zip");
            }
            output
        }
        _ => bytes,
    }
}

struct Row {
    essential: Counts,
    full: Counts,
    skipped: usize,
    files: usize,
}

fn measure_row(
    destination: &Destination,
    sources: &[PathBuf],
    standard_xml: bool,
    options: &Options,
    read_smf_text: bool,
) -> Row {
    let mut row = Row {
        essential: Counts::default(),
        full: Counts::default(),
        skipped: 0,
        files: sources.len(),
    };
    for path in sources {
        let bytes = fs::read(path).expect("read GABC example");
        let score = read(&bytes, Format::Gabc).expect("GABC import");
        let reference = chant_inventory(&score, destination.view);
        let Ok(written) = write(&score, destination.format, options) else {
            row.skipped += 1;
            continue;
        };
        let written = if standard_xml {
            standardise(written, destination.format)
        } else {
            written
        };
        let imported = if read_smf_text {
            scoretk::read_midi_with(
                &written,
                &scoretk::MidiImportOptions {
                    smf_text: true,
                    ..Default::default()
                },
            )
        } else {
            read(&written, destination.format)
        };
        let Ok(imported) = imported else {
            row.skipped += 1;
            continue;
        };
        let candidate = chant_inventory(
            &imported,
            View {
                timeline: Timeline::AsIs,
                ..destination.view
            },
        );
        let all = compare(&reference, &candidate, KINDS);
        let expected: usize = reference
            .iter()
            .filter(|((kind, _, _), _)| *kind == "pitch" || *kind == "bar")
            .map(|(_, count)| count)
            .sum();
        row.essential.reference += expected;
        row.essential.matched += expected
            - all.missing_by_kind.get("pitch").copied().unwrap_or(0)
            - all.missing_by_kind.get("bar").copied().unwrap_or(0);
        let this = compare(&reference, &candidate, destination.declared);
        row.full.reference += this.reference;
        row.full.matched += this.matched;
        row.full.extra += this.extra;
        for (kind, count) in &this.missing_by_kind {
            *row.full.missing_by_kind.entry(*kind).or_insert(0) += count;
        }
    }
    row
}

fn print_row(name: &str, row: &Row, source: &str) {
    println!(
        "{:<19} {:>9.2}% {:>12.2}% {:>6} {:>7} {:>8}  {}",
        name,
        row.essential.percent(),
        row.full.percent(),
        row.files - row.skipped,
        row.skipped,
        row.full.reference,
        source,
    );
    let mut gaps: Vec<(&str, usize)> = row
        .full
        .missing_by_kind
        .iter()
        .map(|(kind, count)| (*kind, *count))
        .collect();
    gaps.sort_by_key(|left| std::cmp::Reverse(left.1));
    if !gaps.is_empty() {
        let parts: Vec<String> = gaps
            .iter()
            .map(|(kind, count)| format!("{kind} {count}"))
            .collect();
        println!("    residual: {}", parts.join(", "));
    }
}

#[test]
fn gabc_to_formats_table() {
    let sources = corpus();
    assert_eq!(sources.len(), 9, "the nine Gregorio examples");
    println!("\n=== GABC → X → model vs. the derived modern view (nine Gregorio examples) ===");
    println!("key = (kind, tick, value); essential = pitch+bar; full = spec-declared kinds");
    println!(
        "{:<19} {:>10} {:>13} {:>6} {:>7} {:>8}  source",
        "destination", "essential", "full-support", "files", "skipped", "ref-items"
    );
    for destination in DESTINATIONS {
        let name = if destination.name == "musicxml" || destination.name == "mxl" {
            format!("{} (standard)", destination.name)
        } else {
            destination.name.to_owned()
        };
        let row = measure_row(destination, &sources, true, &Options::default(), false);
        print_row(
            &name,
            &row,
            destination.spec.split(';').next().unwrap_or(""),
        );
    }
    // The private `gabc:` fields rebuild the L6 payload, hence the written
    // grid of the derived view: an *additive* row, not a replacement.
    for destination in DESTINATIONS.iter().filter(|d| d.name == "musicxml") {
        let private = Destination {
            view: View {
                timeline: Timeline::Written,
                duration: DurationCarrier::Both,
                pauses: false,
                no_ticks: false,
                lyric_text_only: false,
            },
            ..*destination
        };
        let row = measure_row(&private, &sources, false, &Options::default(), false);
        print_row(
            "musicxml (private)",
            &row,
            "same files, scoretk-private gabc: fields restored (L6 → derived view)",
        );
    }
    // Additive : `FF 03` est un meta standard ; l'option `smf_text` ne change
    // que l'écriture de textes, pas les notes (`format-ref/midi.md` §Meta-event
    // types). Elle est opt-in pour ne pas modifier la sortie MIDI par défaut.
    for destination in DESTINATIONS.iter().filter(|d| d.name == "midi") {
        let options = Options {
            midi: scoretk::MidiOptions {
                smf_text: true,
                ..Default::default()
            },
            ..Options::default()
        };
        let row = measure_row(destination, &sources, false, &options, true);
        print_row(
            "midi (smf_text)",
            &row,
            "same files, FF 03/FF 06 text metas enabled",
        );
    }
    println!();
}

/// Diagnostic: first missing and extra items of one source per destination.
/// Never asserts on a percentage; useful to classify a residual as a real
/// carrier gap or a tick/value normalisation issue.
#[test]
fn gabc_to_formats_loss_diagnostic() {
    let sources = corpus();
    let path = &sources[0];
    let bytes = fs::read(path).expect("read GABC example");
    let score = read(&bytes, Format::Gabc).expect("GABC import");
    println!(
        "\n=== GABC → X → model: first losses of {} ===",
        path.file_name().unwrap().to_string_lossy()
    );
    let options = &Options::default();
    for destination in DESTINATIONS {
        let reference = chant_inventory(&score, destination.view);
        let Ok(written) = write(&score, destination.format, options) else {
            println!("{}: write failed", destination.name);
            continue;
        };
        let written = standardise(written, destination.format);
        let Ok(imported) = read(&written, destination.format) else {
            println!("{}: read failed", destination.name);
            continue;
        };
        let candidate = chant_inventory(
            &imported,
            View {
                timeline: Timeline::AsIs,
                ..destination.view
            },
        );
        let mut missing = Vec::new();
        let mut extra = Vec::new();
        let mut actual = candidate.clone();
        for ((kind, tick, value), wanted) in &reference {
            if !destination.declared.contains(kind) {
                continue;
            }
            let key = (*kind, *tick, value.clone());
            let found = actual.get_mut(&key).map_or(0, |have| {
                let taken = (*have).min(*wanted);
                *have -= taken;
                taken
            });
            for _ in found..*wanted {
                missing.push(format!(
                    "{kind} @{} {value}",
                    tick.map(|tick| tick.to_string())
                        .unwrap_or_else(|| "-".into())
                ));
            }
        }
        for ((kind, tick, value), count) in &actual {
            if !destination.declared.contains(kind) {
                continue;
            }
            for _ in 0..*count {
                extra.push(format!(
                    "{kind} @{} {value}",
                    tick.map(|tick| tick.to_string())
                        .unwrap_or_else(|| "-".into())
                ));
            }
        }
        println!(
            "--- {} ({} missing, {} extra) ---",
            destination.name,
            missing.len(),
            extra.len()
        );
        for line in missing.iter().take(15) {
            println!("  - {line}");
        }
        for line in extra.iter().take(15) {
            println!("  + {line}");
        }
    }
    println!();
}

#[test]
fn gabc_to_formats_reference_is_the_derived_view() {
    let sources = corpus();
    let bytes = fs::read(&sources[0]).expect("read GABC example");
    let score = read(&bytes, Format::Gabc).expect("GABC import");
    let inventory = chant_inventory(&score, PLAYED);
    assert!(inventory.keys().any(|(kind, _, _)| *kind == "pitch"));
    assert!(inventory.keys().any(|(kind, _, _)| *kind == "bar"));
    assert!(inventory.keys().any(|(kind, _, _)| *kind == "lyric"));
    assert!(inventory.keys().any(|(kind, _, _)| *kind == "duration"));
}

/// Port SMuFL optionnel des neumes (XSD 4.0 `other-notation/@smufl`).
///
/// Mesure : un glyph `chant*` par lettre chantée, sans champ privé supplémentaire.
/// L'option est additive : la sortie par défaut ne change pas.
#[test]
fn musicxml_smufl_neume_carrier_is_opt_in() {
    let options = Options {
        gabc_neume_smufl: true,
        ..Options::default()
    };
    println!("\n=== MusicXML SMuFL neume carrier (opt-in) ===");
    println!(
        "{:<32} {:>7} {:>7} {:>8}",
        "source", "notes", "glyphs", "distinct"
    );
    let mut total_notes = 0;
    let mut total_glyphs = 0;
    for path in corpus() {
        let bytes = fs::read(&path).expect("gabc");
        let score = read(&bytes, Format::Gabc).expect("import");
        let default = String::from_utf8(
            write(&score, Format::MusicXml, &Options::default()).expect("default XML"),
        )
        .expect("UTF-8");
        assert!(
            !default.contains("smufl=\"chant"),
            "the default output must stay unchanged"
        );
        let xml = String::from_utf8(write(&score, Format::MusicXml, &options).expect("SMuFL XML"))
            .expect("UTF-8");
        let glyphs: Vec<&str> = xml
            .match_indices("smufl=\"")
            .filter_map(|(at, _)| {
                let rest = &xml[at + 7..];
                rest.find('"').map(|end| &rest[..end])
            })
            .filter(|name| name.starts_with("chant"))
            .collect();
        let notes = score.notes.len();
        total_notes += notes;
        total_glyphs += glyphs.len();
        let mut distinct = glyphs.clone();
        distinct.sort_unstable();
        distinct.dedup();
        println!(
            "{:<32} {:>7} {:>7} {:>8}",
            path.file_name().unwrap().to_string_lossy(),
            notes,
            glyphs.len(),
            distinct.len()
        );
        assert_eq!(glyphs.len(), notes, "one SMuFL glyph per sung letter");
        assert!(xml.contains("chantPunctum"), "plain puncta must be carried");
    }
    assert!(total_notes > 0);
    assert_eq!(total_notes, total_glyphs);
    println!("total: {total_glyphs}/{total_notes} glyphs\n");
}
