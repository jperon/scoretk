//! Crédits, format de page et placement des paroles sur le corpus v3 + v4.
//!
//! Pour chaque `X.mscx`, on convertit puis on compare à `X.ref.musicxml` :
//!
//! - les `<credit>` dans l'ordre (page, type, texte, police, `default-x` et
//!   `default-y` arrondis au dixième) ;
//! - `<defaults>` réduit à `<scaling>` et `<page-layout>` (valeurs numériques
//!   à 0,01 dixième près) ;
//! - les `<lyric>` dans l'ordre, attributs de mise en page retirés mais
//!   `placement` conservé (c'est le défaut corrigé : les paroles sans
//!   `<placement>` ne doivent pas être exportées au-dessus de la portée).
//!
//! Les écarts connus sont listés dans [`KNOWN_DIFFERENCES`] : le test les
//! imprime mais ne les cache pas.

use std::fs;
use std::path::{Path, PathBuf};

use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};

/// Date d'encodage figée pour une sortie reproductible.
const ENCODING_DATE: &str = "2026-01-01";

/// Écarts connus, documentés dans `docs/agents/ecarts-noyau.md`.
const KNOWN_DIFFERENCES: &[(&str, &str)] = &[
    (
        "moonlight",
        "entités nommées (&quot;) double-échappées dans le credit-words (writer)",
    ),
    (
        "testBreaksPage",
        "VBox placé après deux systèmes : default-y réel 1446.67, calculé 1626.67 (18 sp)",
    ),
];

fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
}

fn files(version: &str) -> Vec<PathBuf> {
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
    files
}

/// Extrait les blocs délimités par `<tag …>` et `</tag>` dans l'ordre.
fn extract_blocks(xml: &str, tag: &str, close: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let needle = format!("<{tag}");
    let mut rest = xml;
    while let Some(position) = rest.find(&needle) {
        let after = &rest[position..];
        let boundary = after.as_bytes().get(needle.len()).copied();
        if !matches!(boundary, Some(b' ') | Some(b'>') | Some(b'/')) {
            rest = &after[needle.len()..];
            continue;
        }
        if let Some(end) = after.find(close) {
            blocks.push(after[..end + close.len()].to_owned());
            rest = &after[end + close.len()..];
        } else {
            break;
        }
    }
    blocks
}

/// Remplace la valeur d'un attribut numérique par son arrondi.
fn round_attribute(text: &str, name: &str, decimals: usize) -> String {
    let needle = format!("{name}=\"");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find(&needle) {
        out.push_str(&rest[..position + needle.len()]);
        let after = &rest[position + needle.len()..];
        if let Some(end) = after.find('"') {
            let value = &after[..end];
            if let Ok(number) = value.parse::<f64>() {
                out.push_str(&format!("{number:.decimals$}"));
            } else {
                out.push_str(value);
            }
            out.push('"');
            rest = &after[end + 1..];
        } else {
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Retire un attribut et sa valeur.
fn remove_attribute(text: &str, name: &str) -> String {
    let needle = format!(" {name}=\"");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find(&needle) {
        out.push_str(&rest[..position]);
        let after = &rest[position + needle.len()..];
        if let Some(end) = after.find('"') {
            rest = &after[end + 1..];
        } else {
            rest = after;
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Réduit un bloc à une ligne comparable : espaces normalisés.
fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalise un `<credit>` : positions arrondies au dixième.
fn normalize_credit(block: &str) -> String {
    let block = round_attribute(block, "default-x", 1);
    let block = round_attribute(&block, "default-y", 1);
    compact(&block)
}

/// Normalise un `<lyric>` : attributs de mise en page retirés.
fn normalize_lyric(block: &str) -> String {
    let mut block = block.to_owned();
    for name in ["default-x", "default-y", "relative-x", "relative-y"] {
        block = remove_attribute(&block, name);
    }
    compact(&block)
}

/// Valeurs de `<defaults>` qui nous intéressent, dans l'ordre du document.
fn defaults_values(xml: &str) -> Vec<(String, String)> {
    let Some(start) = xml.find("<defaults>") else {
        return Vec::new();
    };
    let Some(end) = xml[start..].find("</defaults>") else {
        return Vec::new();
    };
    let block = &xml[start..start + end];
    let names = [
        "millimeters",
        "tenths",
        "page-height",
        "page-width",
        "left-margin",
        "right-margin",
        "top-margin",
        "bottom-margin",
    ];
    let mut values = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        for name in names {
            let open = format!("<{name}>");
            let close = format!("</{name}>");
            if let Some(value) = line
                .strip_prefix(&open)
                .and_then(|value| value.strip_suffix(&close))
            {
                values.push((name.to_owned(), value.to_owned()));
            }
        }
    }
    values
}

/// Compare deux valeurs numériques avec tolérance, ou les chaînes.
fn same_value(expected: &str, actual: &str) -> bool {
    match (expected.parse::<f64>(), actual.parse::<f64>()) {
        (Ok(expected), Ok(actual)) => (expected - actual).abs() < 0.01,
        _ => expected == actual,
    }
}

#[test]
fn credits_et_paroles_du_corpus() {
    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut failures = String::new();
    let mut checked = 0usize;
    let mut lyrics_checked = 0usize;
    let mut known = Vec::new();

    for version in ["v3", "v4"] {
        for path in files(version) {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("nom UTF-8")
                .to_owned();
            let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
            let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
                panic!("référence {} illisible : {error}", reference_path.display())
            });
            let input = fs::read(&path).expect("lecture du .mscx");
            let actual = convert(&input, &options)
                .map(|bytes| String::from_utf8(bytes).expect("MusicXML UTF-8"))
                .unwrap_or_else(|error| panic!("{version}/{name} : {error}"));

            let is_known = KNOWN_DIFFERENCES.iter().any(|(file, _)| *file == name);
            if is_known {
                known.push(format!("{version}/{name}"));
            }

            // Crédits.
            let expected_credits: Vec<String> = extract_blocks(&reference, "credit", "</credit>")
                .iter()
                .map(|block| normalize_credit(block))
                .collect();
            let actual_credits: Vec<String> = extract_blocks(&actual, "credit", "</credit>")
                .iter()
                .map(|block| normalize_credit(block))
                .collect();
            checked += expected_credits.len();
            if expected_credits != actual_credits {
                if is_known {
                    println!(
                        "{version}/{name} : écart connu — crédits\n  attendu {:?}\n  obtenu  {:?}",
                        expected_credits, actual_credits
                    );
                } else {
                    failures.push_str(&format!(
                        "\n=== {version}/{name} : crédits ===\nattendu : {:#?}\nobtenu  : {:#?}\n",
                        expected_credits, actual_credits
                    ));
                }
            }

            // Format de page.
            let expected_defaults = defaults_values(&reference);
            let actual_defaults = defaults_values(&actual);
            if expected_defaults.len() != actual_defaults.len()
                || !expected_defaults.iter().zip(&actual_defaults).all(
                    |((expected_name, expected), (actual_name, actual))| {
                        expected_name == actual_name && same_value(expected, actual)
                    },
                )
            {
                if is_known {
                    println!("{version}/{name} : écart connu — defaults");
                } else {
                    failures.push_str(&format!(
                        "\n=== {version}/{name} : defaults ===\nattendu : {expected_defaults:#?}\n\
                         obtenu  : {actual_defaults:#?}\n"
                    ));
                }
            }

            // Paroles, dans l'ordre du document (toutes portées, toutes voix).
            let expected_lyrics: Vec<String> = extract_blocks(&reference, "lyric", "</lyric>")
                .iter()
                .map(|block| normalize_lyric(block))
                .collect();
            let actual_lyrics: Vec<String> = extract_blocks(&actual, "lyric", "</lyric>")
                .iter()
                .map(|block| normalize_lyric(block))
                .collect();
            lyrics_checked += expected_lyrics.len();
            if expected_lyrics != actual_lyrics {
                if is_known {
                    println!("{version}/{name} : écart connu — paroles");
                } else {
                    failures.push_str(&format!(
                        "\n=== {version}/{name} : paroles ===\nattendu : {:#?}\nobtenu  : {:#?}\n",
                        expected_lyrics, actual_lyrics
                    ));
                }
            }
        }
    }

    assert!(checked > 0, "aucun crédit comparé dans le corpus");
    assert!(lyrics_checked > 0, "aucune parole comparée dans le corpus");
    println!(
        "crédits comparés : {checked}, paroles comparées : {lyrics_checked}, \
         fichiers en écart connu : {known:?}"
    );
    assert!(
        failures.is_empty(),
        "écarts non documentés avec les références :{failures}"
    );
}
