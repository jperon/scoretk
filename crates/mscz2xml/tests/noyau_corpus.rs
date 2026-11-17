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

//! Comparaison des éléments du noyau musical sur le corpus 3.x et 4.x.
//!
//! Le test extrait des deux documents (sortie de `mscz2xml` et référence
//! `X.ref.musicxml`) uniquement les éléments de la famille noyau, mesure par
//! mesure, et compare les suites de lignes canoniques :
//!
//! - hauteurs (`<pitch>`), pour l'effet des ottavas ;
//! - `<octave-shift>` ;
//! - `<tuplet>` avec `<tuplet-actual>`/`<tuplet-normal>` ;
//! - `<measure-style><multiple-rest>` ;
//! - `<part-name-display>`/`<part-abbreviation-display>` ;
//! - `<volume>`/`<pan>` des canaux MIDI ;
//! - `<tie>`/`<tied>` ;
//! - `<sound tempo>` ;
//! - `<repeat times>`.
//!
//! Les attributs de mise en page (`default-x/y`, `relative-x/y`, `placement`,
//! `width`, polices) sont ignorés. [`FICHIERS`] liste chaque fichier avec le
//! nombre de lignes d'écart attendu : zéro pour un fichier conforme, sinon
//! l'écart est explicite et documenté dans `docs/agents/ecarts-noyau.md`.

use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use similar::{ChangeTag, TextDiff};

use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};

const ENCODING_DATE: &str = "2026-01-01";

/// Fichiers comparés, par corpus, avec le nombre de lignes d'écart attendu.
const FICHIERS: &[(&str, &str, usize)] = &[
    // Corpus 4.x
    ("v4", "copypaste_parts", 0),
    ("v4", "dynamics_niente", 0),
    ("v4", "hairpin_dynamic_inside", 0),
    ("v4", "hairpin_dynamic_jump", 0),
    ("v4", "hairpin_simple", 0),
    ("v4", "linked-ties", 0),
    ("v4", "mmrest", 0),
    ("v4", "part-textlines", 0),
    ("v4", "partExclusion", 0),
    ("v4", "repeat65", 0),
    ("v4", "testExcludeInvisibleElements", 0),
    ("v4", "voiceswitching", 0),
    // Corpus 3.x
    ("v3", "Beam-CrossM1", 0),
    ("v3", "barline01", 0),
    ("v3", "barlinedelete", 0),
    ("v3", "clef-1", 0),
    ("v3", "copypaste_tuplet_01", 0),
    ("v3", "grace", 0),
    ("v3", "keysig_78216", 0),
    ("v3", "layout_elements", 0),
    ("v3", "measure-4", 0),
    ("v3", "moonlight", 0),
    ("v3", "repeat01", 0),
    ("v3", "repeat06", 0),
    ("v3", "repeat07", 0),
    ("v3", "repeat11", 0),
    ("v3", "repeat20", 0),
    ("v3", "repeat39", 0),
    ("v3", "selectionfilter1", 0),
    ("v3", "testChordSymbols", 0),
    ("v3", "testHairpinDynamics", 0),
    ("v3", "testHarmony6", 0),
    ("v3", "testMultiInstrumentPart3", 0),
    ("v3", "testTuplets8", 0),
    ("v3", "testWedge4", 0),
    ("v3", "timesig-05", 0),
    ("v3", "undoResequenceMeasure", 0),
    ("v3", "voices", 0),
];

/// Attributs de mise en page ignorés dans les éléments retenus.
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

fn corpus_path(version: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join(version)
        .join(format!("{name}.mscx"))
}

/// Élément dont le sous-arbre entier est retenu.
fn captured(name: &str, event: &BytesStart<'_>) -> bool {
    match name {
        "pitch"
        | "tuplet"
        | "tie"
        | "tied"
        | "octave-shift"
        | "measure-style"
        | "multiple-rest"
        | "part-name-display"
        | "part-abbreviation-display"
        | "volume"
        | "pan" => true,
        // Le noyau ne garde que le tempo des `<sound>` autonomes.
        "sound" => attribute(event, "tempo").is_some(),
        // Le compteur de lectures d'une barre de fin.
        "repeat" => attribute(event, "times").is_some(),
        _ => false,
    }
}

struct Capture {
    name: String,
    depth: usize,
    buffer: String,
}

/// Réduit un document MusicXML aux éléments du noyau.
fn extract(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut lines = Vec::new();
    let mut capture: Option<Capture> = None;
    let mut depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
                if let Some(current) = &mut capture {
                    current.buffer.push_str(&head(&event));
                } else if name == "measure" {
                    lines.push(measure_head(&event));
                } else if captured(&name, &event) {
                    capture = Some(Capture {
                        name,
                        depth,
                        buffer: head(&event),
                    });
                }
                depth += 1;
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(&event);
                if let Some(current) = &mut capture {
                    current.buffer.push_str(&head(&event));
                    current.buffer.push_str("/>");
                } else if name != "measure" && captured(&name, &event) {
                    let mut line = head(&event);
                    line.push_str("/>");
                    lines.push(line);
                }
            }
            Ok(Event::End(event)) => {
                depth = depth.saturating_sub(1);
                let name = event.name().as_ref().to_owned();
                let finished = capture
                    .as_ref()
                    .is_some_and(|current| depth == current.depth);
                if let Some(current) = &mut capture {
                    if finished {
                        current.buffer.push_str(&format!("</{}>", current.name));
                        let done = capture.take().expect("capture");
                        lines.push(done.buffer);
                    } else {
                        current.buffer.push_str(&format!("</{name}>"));
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(current) = &mut capture {
                    let text = event.xml10_content();
                    if let Ok(text) = quick_xml::escape::unescape(&text) {
                        current.buffer.push_str(text.trim());
                    }
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(current) = &mut capture {
                    current.buffer.push_str(event.as_ref().trim());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML illisible : {error}"),
        }
    }

    lines
}

fn measure_head(event: &BytesStart<'_>) -> String {
    match attribute(event, "number") {
        Some(number) => format!("<measure number=\"{number}\">"),
        None => "<measure>".to_owned(),
    }
}

fn local_name(event: &BytesStart<'_>) -> String {
    event.name().as_ref().to_owned()
}

fn attribute(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (attribute.key.0 == name).then(|| {
            attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map(|value| value.into_owned())
                .unwrap_or_default()
        })
    })
}

/// Ouverture canonique d'un élément, attributs triés.
fn head(event: &BytesStart<'_>) -> String {
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
    head.push('>');
    head
}

fn diff(reference: &[String], actual: &[String]) -> (usize, String) {
    let reference: Vec<&str> = reference.iter().map(String::as_str).collect();
    let actual: Vec<&str> = actual.iter().map(String::as_str).collect();
    let text_diff = TextDiff::from_slices(&reference, &actual);
    let changed = text_diff
        .iter_all_changes()
        .filter(|change| change.tag() != ChangeTag::Equal)
        .count();
    let mut unified = text_diff.unified_diff();
    unified.context_radius(2).header("reference", "sortie");
    (changed, unified.to_string())
}

#[test]
fn elements_du_noyau_conformes() {
    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };

    let mut failures = Vec::new();
    let mut rows = Vec::new();

    for &(version, name, expected) in FICHIERS {
        let path = corpus_path(version, name);
        assert!(
            path.is_file(),
            "fichier du corpus absent : {}",
            path.display()
        );
        let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
        let reference = fs::read_to_string(&reference_path).expect("référence lisible");
        let expected_lines = extract(&reference);

        let input = fs::read(&path).expect("lecture du .mscx");
        let actual = convert(&input, &options).expect("conversion");
        let actual_lines = extract(&String::from_utf8(actual).expect("MusicXML UTF-8"));

        let (changed, diff) = diff(&expected_lines, &actual_lines);
        rows.push(format!("{version}/{name} : {changed} (attendu {expected})"));
        if changed != expected {
            failures.push(format!(
                "{version}/{name} : {changed} lignes d'écart, {expected} attendues\n{diff}"
            ));
        }
    }

    println!("noyau : lignes d'écart par fichier (attendu) :");
    for row in &rows {
        println!("  {row}");
    }
    assert!(
        failures.is_empty(),
        "écarts non attendus :\n{}",
        failures.join("\n")
    );
}
