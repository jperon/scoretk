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

//! Normalisation MusicXML partagée par les tests d'aller-retour.
//!
//! Reprise de `crates/mscz2xml/tests/e2e_corpus.rs` (niveau `Core`) : les deux
//! documents sont réduits à une suite de lignes canoniques avant comparaison.
//! Sont retirés les métadonnées et la mise en page (`<identification>`,
//! `<defaults>`, `<credit>`, `<direction>`, `<stem>`, `<beam>`, `<print>`),
//! les attributs de position et de police, `<barline>` sauf ses `<repeat>` et
//! `<notations>` sauf `tied`/`tuplet`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use similar::{ChangeTag, TextDiff};

/// Éléments dont le sous-arbre entier est ignoré.
const IGNORED_ELEMENTS: &[&str] = &[
    "identification",
    "defaults",
    "credit",
    "direction",
    "stem",
    "beam",
    "print",
];

/// Attributs de mise en page ignorés.
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

/// Décision de filtrage pour un élément.
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

/// Répertoire du corpus d'une version.
pub fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
}

/// Fichiers du corpus d'une version, triés.
pub fn corpus_files(version: &str) -> Vec<PathBuf> {
    let directory = corpus_dir(version);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("corpus introuvable dans {} : {error}", directory.display()))
        .map(|entry| entry.expect("entrée de répertoire").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();
    files
}

fn filter_for(parent: Option<Filter>, name: &str) -> Filter {
    match parent {
        Some(Filter::Drop) => Filter::Drop,
        Some(Filter::Only(kept)) => {
            if kept.contains(&name) {
                Filter::Keep
            } else {
                Filter::Drop
            }
        }
        Some(Filter::Keep) | None => match name {
            _ if IGNORED_ELEMENTS.contains(&name) => Filter::Drop,
            "barline" => Filter::Only(BARLINE_KEPT),
            "notations" => Filter::Only(NOTATIONS_KEPT),
            _ => Filter::Keep,
        },
    }
}

/// Réduit un document MusicXML à ses lignes canoniques.
pub fn normalize(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<(Filter, Option<Node>)> = Vec::new();
    let mut roots: Vec<Entry> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
                let filter = filter_for(stack.last().map(|(filter, _)| *filter), &name);
                let node = matches!(filter, Filter::Keep).then(|| Node {
                    head: format_head(&event),
                    name,
                    self_closing: false,
                    children: Vec::new(),
                });
                stack.push((filter, node));
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(&event);
                let filter = filter_for(stack.last().map(|(filter, _)| *filter), &name);
                if let Filter::Keep = filter {
                    let node = Node {
                        head: format_head(&event),
                        name,
                        self_closing: true,
                        children: Vec::new(),
                    };
                    attach(&mut stack, &mut roots, Entry::Node(node));
                }
            }
            Ok(Event::End(_event)) => {
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

fn local_name(event: &BytesStart<'_>) -> String {
    event.name().as_ref().to_owned()
}

fn text_content(event: &quick_xml::events::BytesText<'_>) -> String {
    let raw = event.xml10_content();
    quick_xml::escape::unescape(&raw)
        .map(|text| text.into_owned())
        .unwrap_or_else(|_| raw.into_owned())
}

fn format_head(event: &BytesStart<'_>) -> String {
    let mut attributes: Vec<(String, String)> = event
        .attributes()
        .filter_map(|attribute| {
            let attribute = attribute.ok()?;
            let key = attribute.key.0.to_owned();
            if IGNORED_ATTRIBUTES.contains(&key.as_str()) {
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

    let mut head = format!("<{}", local_name(event));
    for (key, value) in attributes {
        head.push(' ');
        head.push_str(&key);
        head.push_str("=\"");
        head.push_str(&value);
        head.push('"');
    }
    head
}

/// Nombre de lignes modifiées et diff unifié.
pub fn diff(expected: &[String], actual: &[String]) -> (usize, String) {
    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
    let text_diff = TextDiff::from_slices(&expected_refs, &actual_refs);
    let changed = text_diff
        .iter_all_changes()
        .filter(|change| change.tag() != ChangeTag::Equal)
        .count();
    let mut unified = text_diff.unified_diff();
    unified.context_radius(2).header("reference", "sortie");
    (changed, unified.to_string())
}

/// Tronque un diff pour ne pas noyer la sortie du test.
pub fn truncate(diff: &str, max_lines: usize) -> String {
    let mut lines = diff.lines();
    let kept: Vec<&str> = lines.by_ref().take(max_lines).collect();
    let mut text = kept.join("\n");
    if lines.next().is_some() {
        text.push_str("\n… (diff tronqué)");
    }
    text
}

/// Balises modifiées, pour regrouper les écarts.
pub fn changed_tags(expected: &[String], actual: &[String]) -> Vec<(String, usize)> {
    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
    let text_diff = TextDiff::from_slices(&expected_refs, &actual_refs);
    let mut tags: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for change in text_diff.iter_all_changes() {
        if change.tag() == ChangeTag::Equal {
            continue;
        }
        if let Some(tag) = line_tag(change.value()) {
            *tags.entry(tag).or_default() += 1;
        }
    }
    let mut result: Vec<(String, usize)> = tags.into_iter().collect();
    result.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    result
}

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
