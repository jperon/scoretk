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

//! Aller-retour `.ly` → modèle → MusicXML contre l'export direct du `.mscx`.
//!
//! Les 56 `tests/corpus/ly/*.ly` sont produits par l'écrivain ; relus par le
//! lecteur, ils doivent redonner le même MusicXML que le `.mscx` d'origine.
//! La comparaison reprend les deux niveaux de `e2e_corpus` : noyau musical
//! (bloquant pour [`EXPECTED_CORE`]) et parité complète (bloquant pour
//! [`EXPECTED_FULL`], informatif sinon).
//!
//! ```sh
//! cargo test -p scoretk-lilypond --test read_corpus -- --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// Racine du corpus.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

/// Fichiers dont l'aller-retour `.ly` redonne le noyau musical du `.mscx`.
const EXPECTED_CORE: &[&str] = &[
    "v3_Beam-CrossM1",
    "v3_barlinedelete",
    "v3_copypaste_tuplet_01",
    "v3_grace",
    "v3_keysig_78216",
    "v3_lyricsline01",
    "v3_measure-4",
    "v3_repeat01",
    "v3_repeat06",
    "v3_repeat07",
    "v3_repeat11",
    "v3_repeat20",
    "v3_selectionfilter1",
    "v3_testBreaksPage",
    "v3_testHairpinDynamics",
    "v3_testWedge4",
    "v3_undoResequenceMeasure",
    "v4_dynamics",
    "v4_expression-1",
    "v4_repeat65",
    "v4_repeat66",
    "v4_repeat68",
    "v4_repeat69",
    "v4_testTempoPrecision",
];

/// Fichiers dont l'aller-retour redonne la parité complète.
const EXPECTED_FULL: &[&str] = &["v3_copypaste_tuplet_01", "v3_grace", "v3_lyricsline01"];

/// Éléments ignorés au niveau noyau.
const IGNORED_ELEMENTS: &[&str] = &[
    "identification",
    "defaults",
    "credit",
    "direction",
    "stem",
    "beam",
    "print",
];

/// Attributs ignorés au niveau noyau.
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

/// Enfants conservés d'un élément filtré sélectivement.
const BARLINE_KEPT: &[&str] = &["repeat"];
const NOTATIONS_KEPT: &[&str] = &["tied", "tuplet"];

/// Éléments de mise en page retirés au niveau complet.
const FULL_PAGE_ELEMENTS: &[&str] = &[
    "defaults",
    "print",
    "system-layout",
    "staff-layout",
    "encoding-date",
];

/// Attributs de mise en page retirés au niveau complet.
const FULL_PAGE_ATTRIBUTES: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "width",
    "bezier-x",
    "bezier-y",
];

/// Niveau de normalisation.
#[derive(Clone, Copy)]
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
        _ => match level {
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
                if let Filter::Keep = filter {
                    attach(
                        &mut stack,
                        &mut roots,
                        Entry::Node(Node {
                            head: format_head(&event, level),
                            name,
                            self_closing: true,
                            children: Vec::new(),
                        }),
                    );
                }
            }
            Ok(Event::End(_)) => {
                if let Some((filter, node)) = stack.pop() {
                    if matches!(filter, Filter::Keep) {
                        if let Some(node) = node {
                            attach(&mut stack, &mut roots, Entry::Node(node));
                        }
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if matches!(stack.last(), Some((Filter::Keep, _))) {
                    let text = event.xml10_content();
                    let text = quick_xml::escape::unescape(&text)
                        .map(|text| text.into_owned())
                        .unwrap_or_else(|_| text.into_owned());
                    push_text(&mut stack, text);
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
        [Entry::Text(text)] => lines.push(format!("{indent}{}>{text}</{}>", node.head, node.name)),
        children => {
            lines.push(format!("{indent}{}>", node.head));
            render(children, depth + 1, lines);
            lines.push(format!("{indent}</{}>", node.name));
        }
    }
}

fn format_head(event: &quick_xml::events::BytesStart<'_>, level: Level) -> String {
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
    let mut head = format!("<{}", event.name().as_ref());
    for (key, value) in attributes {
        head.push_str(&format!(" {key}=\"{value}\""));
    }
    head
}

/// Exporte un MusicXML depuis un `.mscx`.
fn export_mscx(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    let container = scoretk_musescore_import::MsczContainer::from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("conteneur de {path:?} : {error}"));
    let score = scoretk_musescore_import::read::read_score(&container)
        .unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    scoretk_musicxml_export::export_musicxml(&score)
        .unwrap_or_else(|error| panic!("export : {error}"))
}

/// Exporte un MusicXML depuis un `.ly`.
fn export_ly(path: &Path) -> String {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    let score = scoretk_lilypond::read_lilypond(&source)
        .unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    scoretk_musicxml_export::export_musicxml(&score)
        .unwrap_or_else(|error| panic!("export : {error}"))
}

/// Fichiers `.ly` du corpus, triés, avec leur `.mscx` d'origine.
fn fichiers() -> Vec<(String, PathBuf, PathBuf)> {
    let mut fichiers = Vec::new();
    for version in ["v3", "v4"] {
        let dossier = corpus().join(version);
        let mut entrees: Vec<PathBuf> = fs::read_dir(&dossier)
            .unwrap_or_else(|error| panic!("lecture de {dossier:?} : {error}"))
            .filter_map(|entree| entree.ok())
            .map(|entree| entree.path())
            .filter(|chemin| {
                chemin
                    .extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        entrees.sort();
        for mscx in entrees {
            let stem = mscx
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("nom");
            let ly = corpus().join("ly").join(format!("{version}_{stem}.ly"));
            if !ly.is_file() {
                continue;
            }
            fichiers.push((format!("{version}_{stem}"), ly, mscx));
        }
    }
    assert!(!fichiers.is_empty(), "corpus vide");
    fichiers
}

/// Compare deux documents et renvoie le nombre de lignes d'écart.
fn diff_count(reference: &[String], produced: &[String]) -> usize {
    let mut differences = reference.len().abs_diff(produced.len());
    for (left, right) in reference.iter().zip(produced) {
        if left != right {
            differences += 1;
        }
    }
    differences
}

#[test]
fn aller_retour_sur_le_corpus() {
    let mut core_equal = 0usize;
    let mut full_equal = 0usize;
    let mut total = 0usize;
    let mut rapports = Vec::new();

    for (name, ly, mscx) in fichiers() {
        total += 1;
        let reference = normalize(&export_mscx(&mscx), Level::Core);
        let produced = normalize(&export_ly(&ly), Level::Core);
        let core = diff_count(&reference, &produced);
        let reference_full = normalize(&export_mscx(&mscx), Level::Full);
        let produced_full = normalize(&export_ly(&ly), Level::Full);
        let full = diff_count(&reference_full, &produced_full);
        if core == 0 {
            core_equal += 1;
        }
        if full == 0 {
            full_equal += 1;
        }
        rapports.push(format!("{name} : noyau {core}, complet {full}"));
        if std::env::var_os("SHOW_DIFF").is_some() && core > 0 {
            let mut shown = 0;
            for (index, (left, right)) in reference.iter().zip(&produced).enumerate() {
                if left != right {
                    eprintln!("{name} [{index}] attendu : {left}");
                    eprintln!("{name} [{index}] produit : {right}");
                    shown += 1;
                    if shown >= 12 {
                        break;
                    }
                }
            }
        }

        if EXPECTED_CORE.contains(&name.as_str()) {
            assert_eq!(core, 0, "{name} : le noyau musical doit être identique");
        }
        if EXPECTED_FULL.contains(&name.as_str()) {
            assert_eq!(full, 0, "{name} : la parité complète doit être identique");
        }
    }

    for rapport in &rapports {
        eprintln!("{rapport}");
    }
    eprintln!("noyau : {core_equal}/{total} identiques ; complet : {full_equal}/{total}");
}
