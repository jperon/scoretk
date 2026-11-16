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

use std::collections::{BTreeMap, BTreeSet};
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

/// Fichiers du corpus 3.x dont la conversion est identique à la référence
/// après normalisation (voir `docs/agents/ecarts-noyau.md`).
const EXPECTED_CONFORMANT_V3: &[&str] = &[
    "copypaste_tuplet_01",
    "lyricsline01",
    "repeat01",
    "repeat06",
    "repeat07",
    "repeat11",
    "repeat20",
    "repeat39",
    "testBreaksPage",
    "testHairpinDynamics",
    "testTboxWords1",
    "testTuplets8",
    "testWedge4",
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
const FULL_PAGE_ATTRIBUTES: &[&str] = &["default-x", "default-y", "relative-x", "relative-y", "width"];

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

fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
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
fn normalize(xml: &str) -> Vec<String> {
    normalize_with(xml, Level::Core)
}

/// Réduit un document MusicXML en ne retirant que la mise en page pure.
fn normalize_full(xml: &str) -> Vec<String> {
    normalize_with(xml, Level::Full)
}

/// Réduit un document MusicXML à ses lignes canoniques.
///
/// Chaque élément retenu devient une ligne ; un élément purement textuel tient
/// sur une seule ligne (`<sign>G</sign>`), ce qui garde les diffs lisibles.
fn normalize_with(xml: &str, level: Level) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Pile des éléments ouverts : filtre + nœud retenu éventuel.
    let mut stack: Vec<(Filter, Option<Node>)> = Vec::new();
    let mut roots: Vec<Entry> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
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
                let name = local_name(&event);
                let filter = filter_for(stack.last().map(|(filter, _)| *filter), &name, level);
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
    run_corpus("v4", EXPECTED_CONFORMANT);
}

#[test]
fn corpus_v3_conforme_aux_references() {
    run_corpus("v3", EXPECTED_CONFORMANT_V3);
}

/// Statistiques d'écart pour une balise.
#[derive(Default)]
struct TagStat {
    lines: usize,
    files: BTreeSet<String>,
    examples: Vec<String>,
}

/// Résultat de la comparaison complète d'un corpus.
struct FullReport {
    /// Nombre de lignes d'écart par fichier, dans l'ordre alphabétique.
    rows: Vec<(String, usize)>,
    /// Écarts groupés par nom de balise, avec exemples.
    tags: BTreeMap<String, TagStat>,
    /// Diffs unifiés, pour le diagnostic.
    diffs: String,
}

/// Extrait le nom de balise d'une ligne canonique.
fn line_tag(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('<')?;
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    let end = rest
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '-' || character == '_'))
        .unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_owned())
}

/// Enregistre les balises des lignes modifiées d'un diff.
fn collect_tags(expected: &[String], actual: &[String], file: &str, tags: &mut BTreeMap<String, TagStat>) {
    let expected_refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
    let text_diff = TextDiff::from_slices(&expected_refs, &actual_refs);
    for change in text_diff.iter_all_changes() {
        if change.tag() == ChangeTag::Equal {
            continue;
        }
        let Some(tag) = line_tag(change.value()) else {
            continue;
        };
        let stat = tags.entry(tag).or_default();
        stat.lines += 1;
        stat.files.insert(file.to_owned());
        let example = change.value().trim();
        let example = if example.chars().count() > 110 {
            format!("{}…", example.chars().take(110).collect::<String>())
        } else {
            example.to_owned()
        };
        if stat.examples.len() < 3 && !stat.examples.contains(&example) {
            stat.examples.push(example);
        }
    }
}

/// Convertit un corpus et compare sa sortie complète (mise en page retirée).
fn run_full_corpus(version: &str) -> FullReport {
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

    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut report = FullReport {
        rows: Vec::new(),
        tags: BTreeMap::new(),
        diffs: String::new(),
    };

    for path in &files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier UTF-8")
            .to_owned();

        let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
        let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
            panic!("référence {} illisible : {error}", reference_path.display())
        });
        let expected = normalize_full(&reference);

        let input = fs::read(path).expect("lecture du .mscx");
        let actual = match convert(&input, &options) {
            Ok(bytes) => normalize_full(&String::from_utf8(bytes).expect("MusicXML UTF-8")),
            Err(error) => {
                report.rows.push((name.clone(), usize::MAX));
                report
                    .diffs
                    .push_str(&format!("\n=== {version}/{name} : ERREUR ===\n{error}\n"));
                continue;
            }
        };

        let (changed, diff) = diff(&expected, &actual);
        report.rows.push((name.clone(), changed));
        if changed > 0 {
            collect_tags(&expected, &actual, &name, &mut report.tags);
            report.diffs.push_str(&format!(
                "\n=== {version}/{name} : {changed} lignes d'écart ===\n"
            ));
            report.diffs.push_str(&truncate_diff(&diff, 60));
            report.diffs.push('\n');
        }
    }

    report
}

#[test]
fn corpus_v4_complet() {
    print_full_report("v4", &run_full_corpus("v4"));
}

#[test]
fn corpus_v3_complet() {
    print_full_report("v3", &run_full_corpus("v3"));
}

/// Imprime le tableau et les écarts groupés d'un corpus.
fn print_full_report(version: &str, report: &FullReport) {
    println!("conformité complète du corpus {version} (mise en page pure retirée) :");
    for (name, changed) in &report.rows {
        if *changed == usize::MAX {
            println!("  {name} : ERREUR");
        } else if *changed == 0 {
            println!("  {name} : identique");
        } else {
            println!("  {name} : {changed} lignes d'écart");
        }
    }
    println!("écarts groupés par balise :");
    print_tag_stats(&report.tags);
    println!("{}", report.diffs);
}

/// Imprime les écarts groupés, triés par nombre de lignes décroissant.
fn print_tag_stats(tags: &BTreeMap<String, TagStat>) {
    let mut stats: Vec<(&String, &TagStat)> = tags.iter().collect();
    stats.sort_by(|(left_name, left), (right_name, right)| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| right.files.len().cmp(&left.files.len()))
            .then_with(|| left_name.cmp(right_name))
    });
    for (tag, stat) in stats {
        let mut examples = stat.examples.join(" | ");
        if examples.is_empty() {
            examples.push_str("(aucun exemple)");
        }
        println!(
            "  {tag} : {} lignes, {} fichiers — {examples}",
            stat.lines,
            stat.files.len()
        );
    }
}

/// Produit le contenu de `docs/agents/ecarts-complets.md`.
///
/// Le test est volontairement non bloquant : il mesure l'écart complet pour
/// suivre la parité, sans exiger qu'elle soit atteinte.
#[test]
fn ecarts_complets_document() {
    let v4 = run_full_corpus("v4");
    let v3 = run_full_corpus("v3");

    println!("---DEBUT DOC---");
    println!("# Écarts complets sur le corpus (parité complète)");
    println!();
    println!(
        "Ce document est produit par le test `ecarts_complets_document` de \
         `crates/mscz2xml/tests/e2e_corpus.rs`. Il compare chaque `.mscx` à sa \
         référence `X.ref.musicxml` après retrait de la **mise en page pure** \
         uniquement : attributs `default-x`, `default-y`, `relative-x`, \
         `relative-y`, `width`, blocs `<defaults>`, `<print>` (et leurs \
         `<system-layout>`/`<staff-layout>`) et `<encoding-date>`. Tout le \
         reste est comparé : `<stem>`, `<beam>`, `<barline>`, `<direction>`, \
         `<notations>`, `<credit>`, `<part-list>`, `<identification>`, etc."
    );
    println!();
    println!("## Tableau par fichier");
    println!();
    for (version, report) in [("v3", &v3), ("v4", &v4)] {
        println!("### Corpus {version}");
        println!();
        println!("| Fichier | Lignes d'écart |");
        println!("| --- | ---: |");
        for (name, changed) in &report.rows {
            if *changed == usize::MAX {
                println!("| {name} | ERREUR |");
            } else if *changed == 0 {
                println!("| {name} | **identique** |");
            } else {
                println!("| {name} | {changed} |");
            }
        }
        println!();
    }

    println!("## Écarts groupés par balise");
    println!();
    println!("Fréquence = nombre de lignes canoniques modifiées sur les deux corpus ; \
              fichiers = nombre de fichiers touchés. Tri par fréquence décroissante.");
    println!();
    println!("| Balise | Lignes | Fichiers | Exemples |");
    println!("| --- | ---: | ---: | --- |");
    let mut combined: BTreeMap<String, TagStat> = BTreeMap::new();
    for report in [&v3, &v4] {
        for (tag, stat) in &report.tags {
            let entry = combined.entry(tag.clone()).or_default();
            entry.lines += stat.lines;
            entry.files.extend(stat.files.iter().cloned());
            for example in &stat.examples {
                if entry.examples.len() < 3 && !entry.examples.contains(example) {
                    entry.examples.push(example.clone());
                }
            }
        }
    }
    let mut stats: Vec<(&String, &TagStat)> = combined.iter().collect();
    stats.sort_by(|(left_name, left), (right_name, right)| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| right.files.len().cmp(&left.files.len()))
            .then_with(|| left_name.cmp(right_name))
    });
    for (tag, stat) in stats {
        let examples = stat
            .examples
            .iter()
            .map(|example| format!("`{example}`"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "| `{tag}` | {} | {} | {} |",
            stat.lines,
            stat.files.len(),
            examples
        );
    }
    println!();
    println!("## Diffs");
    println!();
    println!("```diff");
    print!("{}", v4.diffs);
    print!("{}", v3.diffs);
    println!("```");
    println!("---FIN DOC---");
}

/// Convertit tout un corpus et compare chaque fichier à sa référence.
fn run_corpus(version: &str, expected_conformant: &[&str]) {
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

    println!("conformité du corpus {version} (après normalisation) :");
    for row in &rows {
        println!("  {row}");
    }
    if !diffs.is_empty() {
        println!("{diffs}");
    }

    for name in expected_conformant {
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
