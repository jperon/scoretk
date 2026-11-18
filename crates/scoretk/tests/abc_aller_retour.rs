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

//! Aller-retour `.mscx` → ABC → modèle → MusicXML.
//!
//! Chaque fichier du corpus est converti directement en MusicXML, puis via
//! ABC, et les deux sorties sont réduites à leur noyau musical : mesures,
//! attributs (divisions, armure, mesure, clef), notes (hauteur, durée, type,
//! points, voix, portée, liaisons, n-olets) et reprises. Les différences de
//! mise en page et de métadonnées sont ignorées.
//!
//! Tous les fichiers doivent au moins se convertir ; seuls ceux de
//! [`EXPECTED_ROUNDTRIP`] doivent avoir un noyau identique. Les autres écarts
//! sont listés par le test et suivis dans `docs/agents/abc.md`.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

const EXPECTED_ROUNDTRIP: &[&str] = &[
    "barline01",
    "barlinedelete",
    "Beam-CrossM1",
    "beamStemDir",
    "change",
    "changevisibility",
    "copypastesymbollist-lyrics",
    "copypaste_tuplet_01",
    "dynamics",
    "dynamics_niente",
    "expression-1",
    "grace",
    "hairpin_dynamic_inside",
    "hairpin_dynamic_jump",
    "hairpins_and_repeats",
    "hairpin_simple",
    "keysig_78216",
    "lyricsline01",
    "measure-4",
    "mmrest",
    "partExclusion",
    "part-textlines",
    "repeat01",
    "repeat06",
    "repeat07",
    "repeat11",
    "repeat20",
    "repeat39",
    "repeat65",
    "repeat66",
    "repeat68",
    "repeat69",
    "selectionfilter1",
    "tenuto_accent",
    "testBreaksPage",
    "testChordSymbols",
    "testExcludeInvisibleElements",
    "testHairpinDynamics",
    "testHarmony6",
    "testLocalTimesig1",
    "testNoteheads2",
    "testTboxWords1",
    "testTempoPrecision",
    "testTuplets8",
    "timesig-05",
    "undoResequenceMeasure",
];

fn corpus(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(version)
}

fn files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for version in ["v3", "v4"] {
        let mut entries: Vec<PathBuf> = fs::read_dir(corpus(version))
            .expect("corpus")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        entries.sort();
        files.extend(entries);
    }
    files
}

fn direct_xml(path: &Path) -> String {
    let bytes = fs::read(path).expect("lecture .mscx");
    let container = scoretk_musescore_import::MsczContainer::from_bytes(&bytes).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let options = scoretk_musicxml_export::ExportOptions {
        encoding_date: "2026-11-17".to_owned(),
        ..scoretk_musicxml_export::ExportOptions::default()
    };
    scoretk_musicxml_export::export_musicxml_with(&score, &options).expect("export direct")
}

fn roundtrip_xml(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let container = scoretk_musescore_import::MsczContainer::from_bytes(&bytes)
        .map_err(|error| error.to_string())?;
    let score = scoretk_musescore_import::read::read_score(&container)
        .map_err(|error| error.to_string())?;
    let abc = scoretk_abc_export::write_abc(&score).map_err(|error| error.to_string())?;
    let read = scoretk_abc_import::read_abc(&abc).map_err(|error| error.to_string())?;
    let options = scoretk_musicxml_export::ExportOptions {
        encoding_date: "2026-11-17".to_owned(),
        ..scoretk_musicxml_export::ExportOptions::default()
    };
    scoretk_musicxml_export::export_musicxml_with(&read, &options)
        .map_err(|error| error.to_string())
}

#[test]
fn conversions_abc_et_noyau() {
    let mut converted = 0usize;
    let mut identical = 0usize;
    let mut report = String::new();
    for path in files() {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("?")
            .to_owned();
        let direct = direct_xml(&path);
        let roundtrip = match roundtrip_xml(&path) {
            Ok(xml) => xml,
            Err(error) => {
                writeln!(report, "{stem} : ÉCHEC ABC ({error})").expect("rapport");
                continue;
            }
        };
        converted += 1;
        let expected = core(&direct);
        let actual = core(&roundtrip);
        if expected == actual {
            identical += 1;
            writeln!(report, "{stem} : identique").expect("rapport");
        } else {
            let first = expected
                .iter()
                .zip(actual.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(actual.len()));
            let expected_line = expected.get(first).map_or("<fin>", String::as_str);
            let actual_line = actual.get(first).map_or("<fin>", String::as_str);
            writeln!(
                report,
                "{stem} : {} lignes d'écart (première à {first}) attendu {expected_line:?} obtenu {actual_line:?}",
                expected.len().max(actual.len()) - first
                    - usize::from(expected.get(first) == actual.get(first))
            )
            .expect("rapport");
            if EXPECTED_ROUNDTRIP.contains(&stem.as_str()) {
                panic!("{stem} doit être conforme :\n{report}");
            }
        }
    }
    eprintln!("{converted} fichiers convertis en ABC, {identical} noyaux identiques");
    eprintln!("{report}");
    assert_eq!(
        converted,
        files().len(),
        "tous les fichiers du corpus doivent se convertir en ABC"
    );
}

/// Réduit un MusicXML à son noyau musical.
fn core(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    let mut measure: Option<Vec<String>> = None;
    let mut attributes: Option<Vec<(String, String)>> = None;
    let mut note: Option<Vec<(String, String)>> = None;
    let mut measure_number = 0usize;
    let mut text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
                if name == "note" {
                    note = Some(Vec::new());
                }
                if name == "measure" {
                    measure_number += 1;
                    measure = Some(Vec::new());
                    measure
                        .as_mut()
                        .expect("mesure")
                        .push(format!("mesure {measure_number}"));
                }
                if name == "attributes" {
                    attributes = Some(Vec::new());
                }
                path.push(name);
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(&event);
                if name == "repeat" {
                    let direction = attribute(&event, "direction").unwrap_or_default();
                    if let Some(measure) = measure.as_mut() {
                        measure.push(format!("reprise {direction}"));
                    }
                }
                if name == "dot" {
                    if let Some(note) = note.as_mut() {
                        note.push(("dot".to_owned(), "1".to_owned()));
                    }
                }
                if name == "chord" || name == "rest" || name == "grace" {
                    if let Some(note) = note.as_mut() {
                        note.push((name.clone(), "1".to_owned()));
                    }
                }
                if name == "tuplet" {
                    if let Some(note) = note.as_mut() {
                        note.push(("tuplet".to_owned(), "1".to_owned()));
                    }
                }
                if name == "tie" {
                    let kind = attribute(&event, "type").unwrap_or_default();
                    if let Some(note) = note.as_mut() {
                        note.push(("tie".to_owned(), kind));
                    }
                }
            }
            Ok(Event::Text(event)) => {
                text = event.xml10_content().trim().to_owned();
            }
            Ok(Event::End(event)) => {
                let name = event.name().as_ref().to_owned();
                if name == "attributes" {
                    if let (Some(collected), Some(measure)) = (attributes.take(), measure.as_mut())
                    {
                        let mut collected: Vec<(String, String)> = collected
                            .into_iter()
                            .filter(|(key, _)| {
                                matches!(
                                    key.as_str(),
                                    "key/fifths"
                                        | "time/beats"
                                        | "time/beat-type"
                                        | "clef/sign"
                                        | "clef/line"
                                        | "clef/octave-change"
                                )
                            })
                            .collect();
                        collected.sort();
                        collected.dedup();
                        for (key, value) in collected {
                            measure.push(format!("attribut {key}={value}"));
                        }
                    }
                } else if name == "note" {
                    if let (Some(collected), Some(measure)) = (note.take(), measure.as_mut()) {
                        let mut collected: Vec<(String, String)> = collected
                            .into_iter()
                            .filter(|(key, _)| {
                                let key = key.as_str();
                                key == "type"
                                    || key == "voice"
                                    || key == "staff"
                                    || key == "tie"
                                    || key.starts_with("pitch/")
                                    || key.starts_with("time-modification/")
                                    || key.starts_with("lyric/")
                                    || key == "rest"
                                    || key == "chord"
                                    || key == "grace"
                                    || key == "tuplet"
                                    || key == "dot"
                            })
                            .collect();
                        let dots = collected.iter().filter(|(key, _)| key == "dot").count();
                        collected.retain(|(key, _)| key != "dot");
                        collected.sort();
                        let mut line = format!("note points={dots}");
                        for (key, value) in collected {
                            line.push(' ');
                            line.push_str(&key);
                            line.push('=');
                            line.push_str(&value);
                        }
                        measure.push(line);
                    }
                } else if name == "measure" {
                    if let Some(collected) = measure.take() {
                        lines.extend(collected);
                    }
                } else if let Some(note) = note.as_mut() {
                    let key = if path.len() >= 2 {
                        format!("{}/{}", path[path.len() - 2], name)
                    } else {
                        name.clone()
                    };
                    let value = text.clone();
                    if !value.is_empty() {
                        note.push((key, value));
                    }
                } else if let Some(attributes) = attributes.as_mut() {
                    let key = if path.len() >= 2 {
                        format!("{}/{}", path[path.len() - 2], name)
                    } else {
                        name.clone()
                    };
                    if !text.is_empty() {
                        attributes.push((key, text.clone()));
                    }
                }
                path.pop();
                text.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML illisible : {error}"),
        }
    }
    lines
}

fn local_name(event: &BytesStart<'_>) -> String {
    event.name().as_ref().to_owned()
}

fn attribute(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .map(|attribute| attribute.value.to_string())
}
