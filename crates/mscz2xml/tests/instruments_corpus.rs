// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
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

//! Éléments « instruments et accords chiffrés » sur les corpus 3.x et 4.x.
//!
//! Pour chaque `tests/corpus/{v3,v4}/X.mscx`, ce test lit puis exporte la
//! partition et compare à `X.ref.musicxml` les seuls éléments de la famille :
//!
//! - `<part-list>` : `<score-instrument>` (nom, `<instrument-sound>`),
//!   `<midi-device>`, `<midi-instrument>` (canal, programme, volume, pan) et
//!   `<part-group>` ;
//! - par mesure : le numéro, `<attributes>` (`instruments`, `key`, `clef`,
//!   `transpose`, `staff-details`), `<sound>` (changements d'instrument),
//!   `<harmony>` et, dans chaque note, `<instrument>`, `<staff>`, `<accidental>`.
//!
//! Les attributs de mise en page (`default-x`, `default-y`, `relative-x`,
//! `relative-y`, ainsi que les polices et couleurs) sont ignorés. Les extraits
//! sont comparés dans l'ordre du document, mesure par mesure.
//!
//! `<sound><instrument-change>` (changement d'instrument) appartient à la
//! famille des textes et n'est pas extrait ici ; les écarts qui le concernent
//! sont listés dans `docs/agents/ecarts-noyau.md`.
//!
//! [`ECARTS_ATTENDUS`] liste les fichiers dont un élément de la famille
//! manque encore et l'élément en cause : le test filtre ces éléments avant de
//! comparer, si bien qu'une régression ailleurs fait échouer le test tandis
//! qu'une correction de ces écarts reste possible sans le casser.

use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};

const ENCODING_DATE: &str = "2026-01-01";

/// Écarts restants, par fichier, avec les éléments encore manquants.
///
/// - `<staff-details>` : portée masquée, qui dépend de la mise en page.
const ECARTS_ATTENDUS: &[(&str, &str, &[&str])] = &[("v3", "barline01", &["staff-details"])];

/// Vrai si `name`, enfant de `parent`, ouvre un sous-arbre de la famille.
fn est_racine(name: &str, parent: Option<&str>) -> bool {
    match name {
        "score-instrument" | "midi-device" | "midi-instrument" | "part-group" => true,
        "harmony" => parent == Some("measure"),
        "accidental" | "instrument" | "staff" => parent == Some("note"),
        "key" | "clef" | "transpose" | "staff-details" => parent == Some("attributes"),
        _ => false,
    }
}

/// Éléments feuilles de la famille, avec leur texte direct.
const FEUILLES: &[&str] = &[
    "instrument-name",
    "instrument-sound",
    "midi-channel",
    "midi-program",
    "volume",
    "pan",
    "group-symbol",
    "group-barline",
    "instruments",
    "numeral-root",
    "root-step",
    "root-alter",
    "kind",
    "bass-step",
    "bass-alter",
    "degree-value",
    "degree-alter",
    "degree-type",
    "function",
    "offset",
    "sign",
    "line",
    "diatonic",
    "chromatic",
    "octave-change",
];

/// Attributs ignorés (mise en page).
const IGNORED_ATTRIBUTES: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "width",
    "height",
    "color",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "justify",
    "valign",
    "placement",
    "print-object",
    "print-spacing",
];

fn corpus_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
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

/// Attributs retenus, triés, sans mise en page.
fn head(event: &BytesStart<'_>) -> String {
    let mut attributes: Vec<(String, String)> = event
        .attributes()
        .flatten()
        .filter_map(|attribute| {
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

/// Réduit un MusicXML aux lignes de la famille, mesure par mesure.
fn extrait(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut lines = Vec::new();
    // Profondeur et nom des éléments ouverts.
    let mut stack: Vec<String> = Vec::new();
    // Élément racine de la famille dans lequel on se trouve.
    let mut capture: Option<String> = None;
    // Profondeur de la mesure courante.
    let mut measure_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
                let depth = stack.len();
                if name == "measure" {
                    measure_depth = depth;
                    lines.push(format!("{depth} {}", head(&event)));
                } else if est_racine(&name, stack.last().map(String::as_str)) {
                    capture.get_or_insert_with(|| name.clone());
                    lines.push(format!("{measure_depth} [{name}] {}", head(&event)));
                } else if FEUILLES.contains(&name.as_str()) && capture.is_some() {
                    // Le texte est ajouté à la fermeture, pour gérer les
                    // éléments vides comme les autres.
                    lines.push(format!("{} [{:?}] {}", measure_depth, name, head(&event)));
                }
                stack.push(name);
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(&event);
                if name == "measure" {
                    lines.push(format!("{} {}", stack.len(), head(&event)));
                } else if est_racine(&name, stack.last().map(String::as_str)) {
                    lines.push(format!("{measure_depth} [{name}] {} /", head(&event)));
                } else if FEUILLES.contains(&name.as_str()) && capture.is_some() {
                    lines.push(format!("{} [{:?}] {} /", measure_depth, name, head(&event)));
                }
            }
            Ok(Event::Text(text)) => {
                // Seul le texte direct d'une feuille de la famille compte : le
                // contenu d'un `<words>` voisin ne doit pas entrer dans
                // l'extrait.
                let feuille = stack
                    .last()
                    .is_some_and(|name| FEUILLES.contains(&name.as_str()));
                if capture.is_some() && feuille {
                    let text = text_content(&text);
                    let text = text.trim();
                    if !text.is_empty() {
                        let name = stack.last().map(String::as_str).unwrap_or_default();
                        lines.push(format!("{measure_depth} [{name}] texte={text:?}"));
                    }
                }
            }
            Ok(Event::End(_)) => {
                let name = stack.pop().unwrap_or_default();
                if est_racine(&name, stack.last().map(String::as_str))
                    && capture.as_deref() == Some(name.as_str())
                {
                    capture = None;
                }
                if name == "measure" {
                    measure_depth = usize::MAX;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    lines
}

/// Retire les éléments attendus en écart pour un fichier.
fn filtrer(mut lines: Vec<String>, elements: &[&str]) -> Vec<String> {
    lines.retain(|line| !elements.iter().any(|element| line.contains(element)));
    lines
}

fn ecarts_attendus(version: &str, name: &str) -> Option<&'static [&'static str]> {
    ECARTS_ATTENDUS
        .iter()
        .find(|(corpus, fichier, _)| *corpus == version && *fichier == name)
        .map(|(_, _, elements)| *elements)
}

#[test]
fn elements_instruments_et_harmonies() {
    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut rapport = String::new();
    let mut echecs = Vec::new();

    for version in ["v3", "v4"] {
        let directory = corpus_dir(version);
        let mut fichiers: Vec<PathBuf> = fs::read_dir(&directory)
            .expect("corpus")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        fichiers.sort();

        for path in fichiers {
            let name = path
                .file_stem()
                .expect("nom de fichier")
                .to_string_lossy()
                .into_owned();
            let mscx = fs::read(&path).expect("lecture .mscx");
            let sortie = convert(&mscx, &options).expect("conversion");
            let sortie = String::from_utf8(sortie).expect("UTF-8");
            let reference = fs::read_to_string(directory.join(format!("{name}.ref.musicxml")))
                .expect("référence");

            let mut attendu = extrait(&reference);
            let mut obtenu = extrait(&sortie);
            if let Some(elements) = ecarts_attendus(version, &name) {
                attendu = filtrer(attendu, elements);
                obtenu = filtrer(obtenu, elements);
            }
            if attendu == obtenu {
                continue;
            }
            let premier = attendu
                .iter()
                .zip(&obtenu)
                .position(|(left, right)| left != right);
            let detail = match premier {
                Some(index) => {
                    let from = index.saturating_sub(4);
                    let to = (index + 5).min(attendu.len().max(obtenu.len()));
                    let mut contexte = String::new();
                    for line in from..to {
                        let left = attendu.get(line).map(String::as_str).unwrap_or("<fin>");
                        let right = obtenu.get(line).map(String::as_str).unwrap_or("<fin>");
                        let mark = if line == index { ">" } else { " " };
                        contexte.push_str(&format!(
                            "\n{mark} {:>4} attendu : {left:?}\n{mark}      obtenu  : {right:?}",
                            line + 1
                        ));
                    }
                    format!("\n  première divergence ligne {}{contexte}", index + 1)
                }
                None => format!(
                    "\n  nombre de lignes : {} attendues, {} obtenues",
                    attendu.len(),
                    obtenu.len()
                ),
            };
            echecs.push(format!("{version}/{name}{detail}"));
            rapport.push_str(&format!("{version}/{name} : écart\n"));
        }
    }

    assert!(
        echecs.is_empty(),
        "écarts inattendus :\n{}",
        echecs.join("\n")
    );
}
