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

//! Test d'aller-retour : lire → écrire `.mscx` → relire → exporter.
//!
//! Pour chaque fichier du corpus, la sortie MusicXML du modèle relu doit être
//! identique à celle du modèle d'origine, aux **deux niveaux** du test e2e de
//! `scoretk_core` (noyau musical et parité complète). Les fichiers de
//! `EXPECTED_ROUNDTRIP_*` sont bloquants ; les autres sont mesurés et leurs
//! écarts sont documentés dans `docs/agents/mscx-write.md`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;
use scoretk_musicxml_export::ExportOptions;
use similar::{ChangeTag, TextDiff};

/// Date d'encodage figée, comme dans `e2e_corpus.rs`.
const ENCODING_DATE: &str = "2026-11-17";

/// Fichiers 4.x dont l'aller-retour est identique aux deux niveaux.
const EXPECTED_ROUNDTRIP_V4: &[&str] = &[
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

/// Fichiers 3.x dont l'aller-retour (lu en 3.x, écrit en 4.x) est identique.
const EXPECTED_ROUNDTRIP_V3: &[&str] = &[
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
    "moonlight",
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

/// Éléments ignorés au niveau noyau (copie de `e2e_corpus.rs`).
const IGNORED_ELEMENTS: &[&str] = &[
    "identification",
    "defaults",
    "credit",
    "direction",
    "stem",
    "beam",
    "print",
];

/// Attributs de mise en page ignorés au niveau noyau.
const IGNORED_ATTRIBUTES: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "width",
    "height",
    "print-object",
    "print-spacing",
    "print-leger",
    "justify",
    "valign",
    "placement",
    "color",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
];

/// Éléments retirés au niveau complet.
const FULL_PAGE_ELEMENTS: &[&str] = &[
    "defaults",
    "print",
    "system-layout",
    "staff-layout",
    "encoding-date",
];

/// Attributs de mise en page pure retirés au niveau complet.
const FULL_PAGE_ATTRIBUTES: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "width",
    "bezier-x",
    "bezier-y",
];

const BARLINE_KEPT: &[&str] = &["repeat"];
const NOTATIONS_KEPT: &[&str] = &["tied", "tuplet"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    Core,
    Full,
}

#[derive(Clone, Copy)]
enum Filter {
    Keep,
    Drop,
    Only(&'static [&'static str]),
}

struct Node {
    name: String,
    head: String,
    self_closing: bool,
    children: Vec<Entry>,
}

enum Entry {
    Text(String),
    Node(Node),
}

fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
}

fn filter_for(parent: Option<Filter>, name: &str, level: Level) -> Filter {
    match parent {
        Some(Filter::Drop) => Filter::Drop,
        Some(Filter::Only(kept)) => {
            if kept.contains(&name) {
                Filter::Keep
            } else {
                Filter::Drop
            }
        }
        Some(Filter::Keep) | None => match level {
            Level::Full => {
                if FULL_PAGE_ELEMENTS.contains(&name) {
                    Filter::Drop
                } else {
                    Filter::Keep
                }
            }
            Level::Core => match name {
                _ if IGNORED_ELEMENTS.contains(&name) => Filter::Drop,
                "barline" => Filter::Only(BARLINE_KEPT),
                "notations" => Filter::Only(NOTATIONS_KEPT),
                _ => Filter::Keep,
            },
        },
    }
}

fn normalize(xml: &str, level: Level) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<(Filter, Option<Node>)> = Vec::new();
    let mut roots: Vec<Entry> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = event.name().as_ref().to_owned();
                let filter = filter_for(stack.last().map(|(filter, _)| *filter), &name, level);
                let node = matches!(filter, Filter::Keep).then(|| Node {
                    head: format_head(&event, level),
                    name,
                    self_closing: false,
                    children: Vec::new(),
                });
                stack.push((filter, node));
            }
            Ok(Event::Empty(event)) => {
                let name = event.name().as_ref().to_owned();
                let filter = filter_for(stack.last().map(|(filter, _)| *filter), &name, level);
                if matches!(filter, Filter::Keep) {
                    let node = Node {
                        head: format_head(&event, level),
                        name,
                        self_closing: true,
                        children: Vec::new(),
                    };
                    attach(&mut stack, &mut roots, Entry::Node(node));
                }
            }
            Ok(Event::End(_)) => {
                let (filter, node) = stack.pop().unwrap_or((Filter::Drop, None));
                if matches!(filter, Filter::Keep) {
                    if let Some(node) = node {
                        attach(&mut stack, &mut roots, Entry::Node(node));
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if matches!(stack.last(), Some((Filter::Keep, _))) {
                    push_text(&mut stack, text_content(&event));
                }
            }
            Ok(Event::CData(event)) => {
                if matches!(stack.last(), Some((Filter::Keep, _))) {
                    push_text(&mut stack, event.as_ref().trim().to_owned());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML illisible : {error}"),
        }
    }

    let mut lines = Vec::new();
    render(&roots, 0, &mut lines);
    lines
}

fn attach(stack: &mut [(Filter, Option<Node>)], roots: &mut Vec<Entry>, entry: Entry) {
    for (_, node) in stack.iter_mut().rev() {
        if let Some(node) = node {
            node.children.push(entry);
            return;
        }
    }
    roots.push(entry);
}

fn push_text(stack: &mut [(Filter, Option<Node>)], text: String) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if let Some((_, Some(node))) = stack.last_mut() {
        node.children.push(Entry::Text(text.to_owned()));
    }
}

fn render(entries: &[Entry], depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    for entry in entries {
        match entry {
            Entry::Text(text) => lines.push(format!("{indent}{text}")),
            Entry::Node(node) => render_node(node, depth, lines),
        }
    }
}

fn render_node(node: &Node, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    if node.self_closing {
        lines.push(format!("{indent}{}/>", node.head));
        return;
    }
    match node.children.as_slice() {
        [] => lines.push(format!("{indent}{}></{}>", node.head, node.name)),
        [Entry::Text(text)] => {
            lines.push(format!("{indent}{}>{text}</{}>", node.head, node.name));
        }
        children => {
            lines.push(format!("{indent}{}>", node.head));
            render(children, depth + 1, lines);
            lines.push(format!("{indent}</{}>", node.name));
        }
    }
}

fn text_content(event: &quick_xml::events::BytesText<'_>) -> String {
    let raw = event.xml10_content();
    quick_xml::escape::unescape(&raw)
        .map(|text| text.into_owned())
        .unwrap_or_else(|_| raw.into_owned())
}

fn format_head(event: &BytesStart<'_>, level: Level) -> String {
    let ignored = match level {
        Level::Core => IGNORED_ATTRIBUTES,
        Level::Full => FULL_PAGE_ATTRIBUTES,
    };
    let mut attributes: Vec<(String, String)> = event
        .attributes()
        .filter_map(|attribute| {
            let attribute = attribute.ok()?;
            let key = attribute.key.0.to_owned();
            if ignored.contains(&key.as_str()) {
                return None;
            }
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map(|value| value.into_owned())
                .unwrap_or_default();
            Some((key, value))
        })
        .collect();
    attributes.sort();
    let mut head = format!("<{}", event.name().as_ref().to_owned());
    for (key, value) in attributes {
        head.push(' ');
        head.push_str(&key);
        head.push_str("=\"");
        head.push_str(&value);
        head.push('"');
    }
    head
}

fn changed(expected: &[String], actual: &[String]) -> usize {
    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
    TextDiff::from_slices(&expected_refs, &actual_refs)
        .iter_all_changes()
        .filter(|change| change.tag() != ChangeTag::Equal)
        .count()
}

fn diff(expected: &[String], actual: &[String]) -> String {
    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
    let diff = TextDiff::from_slices(&expected_refs, &actual_refs);
    let mut unified = diff.unified_diff();
    unified.context_radius(2).header("direct", "aller-retour");
    unified
        .to_string()
        .lines()
        .take(40)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Exporte un modèle, avec la date figée du corpus.
fn export(score: &Score) -> String {
    let options = ExportOptions {
        encoding_date: ENCODING_DATE.to_owned(),
        ..ExportOptions::default()
    };
    scoretk_musicxml_export::export_musicxml_with(score, &options).expect("export MusicXML")
}

/// Lit un `.mscx` en mémoire.
fn read(mscx: &[u8]) -> Result<Score, String> {
    let container = MsczContainer::from_bytes(mscx).map_err(|error| error.to_string())?;
    scoretk_musescore_import::read::read_score(&container).map_err(|error| error.to_string())
}

/// Résultat de l'aller-retour d'un fichier.
#[derive(Debug)]
struct Roundtrip {
    core: usize,
    full: usize,
    diff: String,
}

/// Rejoue l'aller-retour d'un fichier du corpus.
///
/// `read` donne le modèle d'origine (lecteur 3.x ou 4.x selon le fichier) ;
/// `mscx` est la sortie de l'écrivain.
fn roundtrip(score: &Score) -> Result<Roundtrip, String> {
    let direct = export(score);
    let mscx = scoretk_mscx::write_mscx(score).map_err(|error| error.to_string())?;
    let reread = read(mscx.as_bytes())?;
    let round = export(&reread);

    let direct_core = normalize(&direct, Level::Core);
    let round_core = normalize(&round, Level::Core);
    let direct_full = normalize(&direct, Level::Full);
    let round_full = normalize(&round, Level::Full);
    let core = changed(&direct_core, &round_core);
    let full = changed(&direct_full, &round_full);
    Ok(Roundtrip {
        core,
        full,
        diff: if core == 0 && full == 0 {
            String::new()
        } else {
            format!(
                "noyau :\n{}\ncomplet :\n{}",
                diff(&direct_core, &round_core),
                diff(&direct_full, &round_full)
            )
        },
    })
}

/// Rejoue l'aller-retour par l'archive `.mscz`.
fn roundtrip_mscz(score: &Score) -> Result<Roundtrip, String> {
    let archive = scoretk_mscx::write_mscz(score).map_err(|error| error.to_string())?;
    let reread = read(&archive)?;
    let direct = export(score);
    let round = export(&reread);
    let direct_core = normalize(&direct, Level::Core);
    let round_core = normalize(&round, Level::Core);
    let direct_full = normalize(&direct, Level::Full);
    let round_full = normalize(&round, Level::Full);
    let core = changed(&direct_core, &round_core);
    let full = changed(&direct_full, &round_full);
    Ok(Roundtrip {
        core,
        full,
        diff: if core == 0 && full == 0 {
            String::new()
        } else {
            format!(
                "noyau :\n{}\ncomplet :\n{}",
                diff(&direct_core, &round_core),
                diff(&direct_full, &round_full)
            )
        },
    })
}

fn run_corpus(version: &str, expected: &[&str]) {
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

    let mut identical = BTreeSet::new();
    let mut rows = Vec::new();
    let mut diffs = String::new();

    for path in &files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier UTF-8")
            .to_owned();
        let input = fs::read(path).expect("lecture du .mscx");
        let score = match read(&input) {
            Ok(score) => score,
            Err(error) => {
                rows.push(format!("{name} : LECTURE IMPOSSIBLE ({error})"));
                continue;
            }
        };
        match (roundtrip(&score), roundtrip_mscz(&score)) {
            (Ok(result), Ok(archive))
                if result.core == 0
                    && result.full == 0
                    && archive.core == 0
                    && archive.full == 0 =>
            {
                identical.insert(name.clone());
                rows.push(format!("{name} : identique (.mscx et .mscz)"));
            }
            (Ok(result), Ok(archive)) => {
                rows.push(format!(
                    "{name} : {} lignes (noyau), {} lignes (complet),                      .mscz {} / {}",
                    result.core, result.full, archive.core, archive.full
                ));
                diffs.push_str(&format!(
                    "\n=== {version}/{name} : {} noyau / {} complet ===\n{}\n",
                    result.core, result.full, result.diff
                ));
                if archive.core != 0 || archive.full != 0 {
                    diffs.push_str(&format!(
                        "\n=== {version}/{name} (.mscz) : {} noyau / {} complet ===\n{}\n",
                        archive.core, archive.full, archive.diff
                    ));
                }
            }
            (Err(error), _) | (_, Err(error)) => {
                rows.push(format!("{name} : ERREUR ({error})"));
            }
        }
    }

    println!("aller-retour .mscx du corpus {version} :");
    for row in &rows {
        println!("  {row}");
    }
    if !diffs.is_empty() {
        println!("{diffs}");
    }

    for name in expected {
        assert!(
            files.iter().any(|path| path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem == *name)),
            "{name} est listé dans EXPECTED mais absent du corpus {version}"
        );
        assert!(
            identical.contains(*name),
            "{name} est attendu identique après aller-retour mais diffère"
        );
    }
}

#[test]
fn aller_retour_v4() {
    run_corpus("v4", EXPECTED_ROUNDTRIP_V4);
}

#[test]
fn aller_retour_v3() {
    run_corpus("v3", EXPECTED_ROUNDTRIP_V3);
}

#[test]
fn archive_mscz_relue() {
    let input = fs::read(corpus_dir("v4").join("dynamics.mscx")).expect("corpus");
    let score = read(&input).expect("lecture");
    let result = roundtrip_mscz(&score).expect("archive .mscz relue");
    assert_eq!((result.core, result.full), (0, 0), "{}", result.diff);
}
