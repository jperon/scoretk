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

//! Normalisation « noyau » d'un document MusicXML, partagée entre le test de
//! bout en bout `e2e_corpus` et l'oracle d'écriture `scripts/oracle/musicxml`.
//! Portage direct de la logique de `crates/scoretk/tests/e2e_corpus.rs` : aucun
//! comportement n'est modifié, seul l'emplacement du code change.

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

/// Niveau de normalisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    /// Noyau musical (voir [`IGNORED_ELEMENTS`]).
    Core,
    /// Parité complète : seule la mise en page pure est retirée.
    Full,
}

/// Éléments de mise en page retirés au niveau [`Level::Full`].
///
/// `<defaults>` regroupe le format de page, le `scaling`, l'apparence des
/// traits et les polices ; `<print>` ne porte que des sauts et des
/// `system-layout`/`staff-layout` ; `<encoding-date>` est la seule partie non
/// reproductible de `<identification>`.
const FULL_PAGE_ELEMENTS: &[&str] = &[
    "defaults",
    "print",
    "system-layout",
    "staff-layout",
    "encoding-date",
];

/// Attributs de mise en page pure retirés au niveau [`Level::Full`].
///
/// `bezier-x`/`bezier-y` sont les points de contrôle des courbes de liaison et
/// de glissando : géométrie calculée par la mise en page, sans équivalent dans
/// le modèle.
const FULL_PAGE_ATTRIBUTES: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "width",
    "bezier-x",
    "bezier-y",
];

/// Décision de filtrage pour un élément.
#[derive(Clone, Copy)]
enum Filter {
    /// Garder l'élément et ses descendants.
    Keep,
    /// Ignorer l'élément et tout son sous-arbre.
    Drop,
    /// Garder uniquement les enfants dont le nom est dans la liste.
    Only(&'static [&'static str]),
}

/// Élément retenu, avec ses enfants.
struct Node {
    name: String,
    /// `<nom attr="valeur" …` sans chevron fermant, attributs triés.
    head: String,
    self_closing: bool,
    children: Vec<Entry>,
}

enum Entry {
    Text(String),
    Node(Node),
}

/// Décide du sort d'un élément d'après son parent, son nom et le niveau.
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

/// Réduit un document MusicXML à ses lignes canoniques, au niveau noyau.
pub fn normalize(xml: &str) -> Vec<String> {
    normalize_core_with_stats(xml).0
}

/// Normalise le noyau et compte les `<staff-details>` masqués écartés.
///
/// `writeMeasure` (`exportmusicxml.cpp:8694-8707`) écrit un
/// `<staff-details print-object="no"/>` par `m_hiddenStaves` au début de
/// chaque système. Ce vecteur vient de `print()` (`:7559-7575`) : il y a
/// notamment les `SysStaff` dont `show()` est faux, donc des éléments de mise
/// en page système et non des propriétés de portée. `writeStaffDetails`
/// (`:8059-8075`) applique le même critère `!st->show()`. Ces blocs sont
/// classés comme différence de mise en page, et comptés séparément pour ne
/// pas les rendre invisibles.
pub fn normalize_core_with_stats(xml: &str) -> (Vec<String>, usize) {
    let (lines, hidden) = normalize_with_stats(xml, Level::Core);
    (lines, hidden)
}

/// Réduit un document MusicXML en ne retirant que la mise en page pure.
pub fn normalize_full(xml: &str) -> Vec<String> {
    normalize_with_stats(xml, Level::Full).0
}

/// Réduit un document MusicXML à ses lignes canoniques.
///
/// Chaque élément retenu devient une ligne ; un élément purement textuel tient
/// sur une seule ligne (`<sign>G</sign>`), ce qui garde les diffs lisibles.
/// Vrai pour un `<staff-details print-object="no">`, c'est-à-dire un
/// `SysStaff` non affiché décrit par `print()`/`writeMeasure` (voir
/// [`normalize_core_with_stats`]).
fn is_hidden_staff_details(event: &quick_xml::events::BytesStart<'_>) -> bool {
    if local_name(event) != "staff-details" {
        return false;
    }
    event.attributes().flatten().any(|attribute| {
        attribute.key.as_ref() == "print-object" && attribute.value.as_ref() == "no"
    })
}

/// Un `<attributes></attributes>` vide ne porte aucune information musicale :
/// il ne reste qu'une enveloppe après le filtrage des blocs de mise en page.
fn prune_empty_attributes(entries: &mut Vec<Entry>) {
    entries.retain(|entry| match entry {
        Entry::Node(node) => node.name != "attributes" || !node.children.is_empty(),
        Entry::Text(_) => true,
    });
    for entry in entries.iter_mut() {
        if let Entry::Node(node) = entry {
            prune_empty_attributes(&mut node.children);
        }
    }
}

fn normalize_with_stats(xml: &str, level: Level) -> (Vec<String>, usize) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Pile des éléments ouverts : filtre + nœud retenu éventuel.
    let mut stack: Vec<(Filter, Option<Node>)> = Vec::new();
    let mut roots: Vec<Entry> = Vec::new();
    let mut hidden_staves = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
                let hidden = level == Level::Core && is_hidden_staff_details(&event);
                if hidden {
                    hidden_staves += 1;
                }
                let filter = if hidden {
                    Filter::Drop
                } else {
                    filter_for(stack.last().map(|(filter, _)| *filter), &name, level)
                };
                let node = matches!(filter, Filter::Keep).then(|| Node {
                    head: format_head(&event, level),
                    name,
                    self_closing: false,
                    children: Vec::new(),
                });
                stack.push((filter, node));
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(&event);
                let hidden = level == Level::Core && is_hidden_staff_details(&event);
                if hidden {
                    hidden_staves += 1;
                }
                let filter = if hidden {
                    Filter::Drop
                } else {
                    filter_for(stack.last().map(|(filter, _)| *filter), &name, level)
                };
                if let Filter::Keep = filter {
                    let node = Node {
                        head: format_head(&event, level),
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

    if level == Level::Core {
        prune_empty_attributes(&mut roots);
    }
    let mut lines = Vec::new();
    render(&roots, 0, &mut lines);
    (lines, hidden_staves)
}

/// Ajoute une entrée à l'élément retenu le plus proche dans la pile, ou à la
/// racine.
///
/// La pile peut contenir des niveaux `Only` sans nœud (par exemple
/// `<barline>` : seuls ses `<repeat>` sont retenus) : l'entrée doit alors
/// remonter jusqu'au dernier nœud conservé, pas sortir à la racine.
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

/// Texte décodé et débarrassé de ses espaces de bord.
fn text_content(event: &quick_xml::events::BytesText<'_>) -> String {
    let raw = event.xml10_content();
    quick_xml::escape::unescape(&raw)
        .map(|text| text.into_owned())
        .unwrap_or_else(|_| raw.into_owned())
}

/// Écrit l'ouverture d'un élément, attributs triés et attributs de mise en
/// page retirés.
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

/// Compte les lignes modifiées et rend le diff unifié lisible.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributs_vides_ecartes_du_noyau() {
        let xml = r#"<score-partwise version="4.0"><part id="P1"><measure number="1">
          <attributes></attributes>
          <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>
        </measure></part></score-partwise>"#;
        let lines = normalize(xml);
        assert!(
            !lines.iter().any(|line| line.contains("attributes")),
            "une enveloppe vide ne porte aucune information :\n{lines:?}"
        );
        assert!(lines.iter().any(|line| line.contains("<step>C</step>")));
    }

    #[test]
    fn staff_details_masque_est_une_difference_de_mise_en_page() {
        let xml = r#"<score-partwise version="4.0"><part id="P1"><measure number="1">
          <attributes>
            <divisions>1</divisions>
            <staff-details print-object="no"><staff-lines>5</staff-lines></staff-details>
            <staff-details><staff-size scaling="70">70</staff-size></staff-details>
          </attributes>
        </measure></part></score-partwise>"#;
        let (lines, hidden) = normalize_core_with_stats(xml);
        assert_eq!(hidden, 1, "un seul bloc masqué doit être compté");
        assert!(
            lines.iter().any(|line| line.contains("<staff-size")),
            "les propriétés réelles de portée restent dans le noyau"
        );
        assert!(
            !lines.iter().any(|line| line.contains("<staff-lines")),
            "le bloc print-object=no ne doit pas rester dans le noyau"
        );
    }
}
