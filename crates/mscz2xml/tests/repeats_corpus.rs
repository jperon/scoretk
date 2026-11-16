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

//! Test d'intégration des reprises : corpus 3.x et 4.x contre les références
//! MusicXML de MuseScore 4.7.4.
//!
//! Pour chaque fichier concerné, on lit le `.mscx`, on l'exporte, puis on
//! compare **mesure par mesure et partie par partie** les seuls éléments de la
//! famille reprises :
//!
//! - `<barline>` (avec `<bar-style>`, `<ending>`, `<repeat>`) ;
//! - `<direction>` qui porte un marqueur ou un saut (`<segno>`, `<coda>`,
//!   `<words>` accompagnés d'un `<sound segno= coda= fine= tocoda= dacapo=
//!   dalsegno=>`).
//!
//! Les attributs de mise en page (`default-x`, `default-y`, `relative-x`,
//! `relative-y`, largeurs, polices) sont retirés de la comparaison : le modèle
//! ne les porte pas. Les écarts attendus sont listés dans
//! [`ECARTS_ATTENDUS`] ; un fichier listé qui passe fait échouer le test, pour
//! éviter une liste périmée.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use mscz2xml::export::export_musicxml;
use mscz2xml::read::read_score;
use mscz2xml::MsczContainer;

/// Fichiers du corpus couverts par la famille reprises.
const FICHIERS: &[(&str, &str)] = &[
    ("v3", "repeat01"),
    ("v3", "repeat06"),
    ("v3", "repeat07"),
    ("v3", "repeat11"),
    ("v3", "repeat20"),
    ("v3", "repeat39"),
    ("v3", "barline01"),
    ("v3", "barlinedelete"),
    ("v4", "repeat65"),
    ("v4", "repeat66"),
    ("v4", "repeat68"),
    ("v4", "repeat69"),
    ("v4", "hairpins_and_repeats"),
    ("v4", "part-textlines"),
    ("v4", "copypaste_parts"),
    ("v4", "mmrest"),
];

/// Écarts connus, avec leur justification. Un fichier listé ici est autorisé à
/// différer ; s'il ne diffère plus, le test échoue pour forcer la mise à jour.
const ECARTS_ATTENDUS: &[(&str, &str)] = &[];

/// Attributs de mise en page absents du modèle.
const ATTRIBUTS_IGNORES: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "width",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "valign",
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
}

/// Extraction normalisée des éléments de reprise, par `(part, mesure)`.
type Extraits = BTreeMap<(usize, usize), Vec<String>>;

struct Capture {
    depth: usize,
    text: String,
    tags: Vec<String>,
}

/// Extrait les éléments de reprise d'un document MusicXML.
fn extraire(xml: &str) -> Extraits {
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(true);

    let mut extraits: Extraits = BTreeMap::new();
    let mut part = 0usize;
    let mut measure = 0usize;
    let mut capture: Option<Capture> = None;

    let finaliser = |capture: &Capture, part: usize, measure: usize, out: &mut Extraits| {
        if !interessant(capture) {
            return;
        }
        out.entry((part, measure))
            .or_default()
            .push(capture.text.trim().to_owned());
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = event.name().0.to_owned();
                if name == "part" {
                    part += 1;
                    measure = 0;
                } else if name == "measure" {
                    measure += 1;
                }
                let tag = normaliser_debut(&event, false);
                if let Some(capture) = capture.as_mut() {
                    capture.text.push_str(&tag);
                    capture.depth += 1;
                    capture.tags.push(name);
                } else if name == "barline" || name == "direction" {
                    capture = Some(Capture {
                        depth: 1,
                        text: tag,
                        tags: vec![name],
                    });
                }
            }
            Ok(Event::Empty(event)) => {
                let name = event.name().0.to_owned();
                let tag = normaliser_debut(&event, true);
                if let Some(capture) = capture.as_mut() {
                    capture.text.push_str(&tag);
                    capture.tags.push(name);
                } else if name == "barline" || name == "direction" {
                    let single = Capture {
                        depth: 1,
                        text: tag,
                        tags: vec![name],
                    };
                    finaliser(&single, part, measure, &mut extraits);
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(capture) = capture.as_mut() {
                    let raw = text.xml_content(quick_xml::XmlVersion::Implicit1_0);
                    capture.text.push_str(&normaliser_texte(&raw));
                }
            }
            Ok(Event::End(event)) => {
                let name = event.name().0.to_owned();
                if let Some(active) = capture.as_mut() {
                    active.text.push_str("</");
                    active.text.push_str(&name);
                    active.text.push('>');
                    active.depth -= 1;
                    if active.depth == 0 {
                        let done = capture.take().expect("capture en cours");
                        finaliser(&done, part, measure, &mut extraits);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML illisible : {error}"),
        }
    }

    extraits
}

/// Écrit un élément avec ses attributs utiles, triés.
fn normaliser_debut(event: &quick_xml::events::BytesStart<'_>, empty: bool) -> String {
    let name = event.name().0.to_owned();
    let mut attributes: Vec<String> = Vec::new();
    for attribute in event.attributes().flatten() {
        let key = attribute.key.0.to_owned();
        if ATTRIBUTS_IGNORES.contains(&key.as_str()) {
            continue;
        }
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map(|value| value.into_owned())
            .unwrap_or_default();
        attributes.push(format!("{key}={:?}", value.trim()));
    }
    attributes.sort();
    let mut out = String::new();
    out.push('<');
    out.push_str(&name);
    for attribute in attributes {
        out.push(' ');
        out.push_str(&attribute);
    }
    out.push_str(if empty { "/>" } else { ">" });
    out
}

/// Réduit les blancs d'un texte, comme le fait MusicXML pour `<words>`.
fn normaliser_texte(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Vrai si la capture est un élément de la famille reprises.
fn interessant(capture: &Capture) -> bool {
    if capture.tags.first().is_some_and(|tag| tag == "barline") {
        return true;
    }
    if capture
        .tags
        .iter()
        .any(|tag| tag == "segno" || tag == "coda")
    {
        return true;
    }
    capture.text.contains("<sound")
        && [
            "segno=",
            "coda=",
            "fine=",
            "tocoda=",
            "dacapo=",
            "dalsegno=",
        ]
        .iter()
        .any(|attribute| capture.text.contains(attribute))
}

/// Compare deux extraits et renvoie les écarts lisibles.
fn comparer(attendu: &Extraits, obtenu: &Extraits) -> Vec<String> {
    let mut cles: Vec<(usize, usize)> = attendu.keys().chain(obtenu.keys()).copied().collect();
    cles.sort_unstable();
    cles.dedup();

    let mut ecarts = Vec::new();
    for cle in cles {
        let left = attendu.get(&cle).cloned().unwrap_or_default();
        let right = obtenu.get(&cle).cloned().unwrap_or_default();
        if left == right {
            continue;
        }
        ecarts.push(format!(
            "partie {} mesure {} :\n  attendu : {left:#?}\n  obtenu  : {right:#?}",
            cle.0, cle.1
        ));
    }
    ecarts
}

fn exporter(chemin: &Path) -> String {
    let data = fs::read(chemin)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", chemin.display()));
    let container = MsczContainer::from_bytes(&data)
        .unwrap_or_else(|error| panic!("conteneur {} invalide : {error}", chemin.display()));
    let score = read_score(&container)
        .unwrap_or_else(|error| panic!("lecture de {} échouée : {error}", chemin.display()));
    export_musicxml(&score)
        .unwrap_or_else(|error| panic!("export de {} échoué : {error}", chemin.display()))
}

#[test]
fn reprises_du_corpus() {
    let mut echecs = Vec::new();

    for (repertoire, nom) in FICHIERS {
        let base = corpus_dir().join(repertoire).join(nom);
        let mscx = base.with_extension("mscx");
        let reference = base.with_extension("ref.musicxml");

        let obtenu = extraire(&exporter(&mscx));
        let attendu = extraire(
            &fs::read_to_string(&reference)
                .unwrap_or_else(|error| panic!("lecture de {} : {error}", reference.display())),
        );
        assert!(
            !attendu.is_empty(),
            "{repertoire}/{nom} : aucun élément de reprise dans la référence"
        );
        assert!(
            !obtenu.is_empty(),
            "{repertoire}/{nom} : aucun élément de reprise dans l'export"
        );
        let ecarts = comparer(&attendu, &obtenu);
        let autorise = ECARTS_ATTENDUS
            .iter()
            .any(|(fichier, _)| *fichier == format!("{repertoire}/{nom}"));

        match (ecarts.is_empty(), autorise) {
            (true, false) => {}
            (true, true) => echecs.push(format!(
                "{repertoire}/{nom} : listé dans ECARTS_ATTENDUS mais identique à la référence"
            )),
            (false, true) => {
                eprintln!("{repertoire}/{nom} : écart attendu (voir ECARTS_ATTENDUS)");
            }
            (false, false) => echecs.push(format!(
                "{repertoire}/{nom} : {} écart(s)\n{}",
                ecarts.len(),
                ecarts.join("\n")
            )),
        }
    }

    assert!(
        echecs.is_empty(),
        "écarts de reprises par rapport aux références MuseScore :\n{}",
        echecs.join("\n\n")
    );
}

/// Garde-fou : vérifie que l'extraction voit bien les marqueurs, les sauts et
/// les endings, et pas seulement les barres de mesure.
#[test]
fn extraction_couvre_marqueurs_et_sauts() {
    let lire = |chemin: &str| {
        let mut xml = fs::read_to_string(corpus_dir().join(chemin))
            .unwrap_or_else(|error| panic!("lecture de {chemin} : {error}"));
        xml.make_ascii_lowercase();
        extraire(&xml)
            .values()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    };

    let fine = lire("v3/repeat07.ref.musicxml");
    assert!(fine.contains("fine=\"yes\""), "{fine}");
    assert!(fine.contains("dacapo=\"yes\""), "{fine}");

    let segno = lire("v3/repeat11.ref.musicxml");
    assert!(segno.contains("<segno"), "{segno}");
    assert!(segno.contains("tocoda="), "{segno}");
    assert!(segno.contains("<coda"), "{segno}");

    let times = lire("v4/repeat65.ref.musicxml");
    assert!(times.contains("times=\"3\""), "{times}");
    assert!(times.contains("ending"), "{times}");
}
