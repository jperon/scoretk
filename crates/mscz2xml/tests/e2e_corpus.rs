//! Test de bout en bout sur le corpus MuseScore 4.
//!
//! Pour chaque `tests/corpus/v4/X.mscx`, on convertit le fichier avec
//! [`mscz2xml::convert`] et on compare le MusicXML obtenu à la référence
//! `X.ref.musicxml` exportée par MuseScore 4.7.4.
//!
//! La comparaison porte sur le **noyau musical** : les deux documents sont
//! d'abord normalisés, c'est-à-dire réduits à une suite de lignes canoniques.
//! La normalisation retire :
//!
//! - les métadonnées et la mise en page : `<identification>`, `<defaults>`,
//!   `<credit>` et les attributs `default-x`, `default-y`, `relative-x`,
//!   `relative-y`, `width`, `print-object`, etc. ;
//! - les éléments hors du périmètre du noyau : `<direction>`, `<stem>`,
//!   `<beam>`, `<print>`, `<notations>` sauf `tied`/`tuplet`, `<barline>` sauf
//!   les reprises (`<repeat>`).
//!
//! Le test affiche un tableau « fichier : identique / N lignes d'écart » et
//! n'échoue que pour les fichiers de [`EXPECTED_CONFORMANT`] : les autres
//! écarts sont documentés dans `docs/agents/ecarts-noyau.md`.

use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use similar::{ChangeTag, TextDiff};

use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};

/// Date d'encodage figée : `<identification>` est normalisé de toute façon,
/// mais la sortie reste ainsi entièrement reproductible.
const ENCODING_DATE: &str = "2026-01-01";

/// Fichiers dont la conversion est aujourd'hui identique à la référence après
/// normalisation. Toute régression sur cette liste fait échouer le test ; les
/// fichiers absents sont suivis dans `docs/agents/ecarts-noyau.md`.
const EXPECTED_CONFORMANT: &[&str] = &[
    "beamStemDir",
    "changevisibility",
    "copypastesymbollist-lyrics",
    "dynamics",
    "expression-1",
    "hairpins_and_repeats",
    "linked-ties",
    "repeat68",
    "repeat69",
    "tenuto_accent",
    "testLocalTimesig1",
    "testNoteheads2",
    "testTempoPrecision",
    "timesig-09",
];

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

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4")
}

/// Décide du sort d'un élément d'après son parent et son nom.
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
///
/// Chaque élément retenu devient une ligne ; un élément purement textuel tient
/// sur une seule ligne (`<sign>G</sign>`), ce qui garde les diffs lisibles.
fn normalize(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Pile des éléments ouverts : filtre + nœud retenu éventuel.
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
                if let Filter::Keep = filter_for(stack.last().map(|(filter, _)| *filter), &name) {
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

/// Compte les lignes modifiées et rend le diff unifié lisible.
fn diff(expected: &[String], actual: &[String]) -> (usize, String) {
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
fn truncate_diff(diff: &str, max_lines: usize) -> String {
    let mut lines = diff.lines();
    let kept: Vec<&str> = lines.by_ref().take(max_lines).collect();
    let mut text = kept.join("\n");
    if lines.next().is_some() {
        text.push_str("\n… (diff tronqué)");
    }
    text
}

#[test]
fn corpus_v4_conforme_aux_references() {
    let directory = corpus_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("corpus introuvable dans {} : {error}", directory.display()))
        .map(|entry| entry.expect("entrée de répertoire").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();

    assert!(!files.is_empty(), "aucun fichier .mscx dans le corpus v4");

    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut names = Vec::new();
    let mut identical = Vec::new();
    let mut rows = Vec::new();
    let mut diffs = String::new();

    for path in &files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier UTF-8")
            .to_owned();
        names.push(name.clone());

        let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
        let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
            panic!("référence {} illisible : {error}", reference_path.display())
        });
        let expected = normalize(&reference);

        let input = fs::read(path).expect("lecture du .mscx");
        let actual = match convert(&input, &options) {
            Ok(bytes) => normalize(&String::from_utf8(bytes).expect("MusicXML UTF-8")),
            Err(error) => {
                rows.push(format!("{name} : ERREUR ({error})"));
                diffs.push_str(&format!("\n=== {name} : ERREUR ===\n{error}\n"));
                continue;
            }
        };

        let (changed, diff) = diff(&expected, &actual);
        if changed == 0 {
            identical.push(name.clone());
            rows.push(format!("{name} : identique"));
        } else {
            rows.push(format!("{name} : {changed} lignes d'écart"));
            diffs.push_str(&format!("\n=== {name} : {changed} lignes d'écart ===\n"));
            diffs.push_str(&truncate_diff(&diff, 80));
            diffs.push('\n');
        }
    }

    println!("conformité du corpus v4 (après normalisation) :");
    for row in &rows {
        println!("  {row}");
    }
    if !diffs.is_empty() {
        println!("{diffs}");
    }

    for name in EXPECTED_CONFORMANT {
        assert!(
            names.iter().any(|candidate| candidate == name),
            "{name} est listé dans EXPECTED_CONFORMANT mais absent du corpus"
        );
        assert!(
            identical.iter().any(|candidate| candidate == name),
            "{name} est attendu conforme mais diffère de sa référence"
        );
    }
}
