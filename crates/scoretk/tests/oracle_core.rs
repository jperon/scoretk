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

//! Aide de l'oracle d'écriture MusicXML : compare deux documents avec la
//! normalisation `core`/`full` déjà utilisée par `e2e_corpus`.
//!
//! Ce test est ignoré par défaut. L'oracle le lance avec :
//!
//! ```sh
//! SCORETK_ORACLE_MANIFEST=... SCORETK_ORACLE_OUTPUT=... \
//!   cargo test -p scoretk --test oracle_core -- --ignored
//! ```
//!
//! Le manifeste contient une ligne `identifiant<tab>référence<tab>sortie`.
//! Chaque ligne de résultat est `identifiant<tab>lignes-noyau<tab>lignes-plein<tab>tag:count,…`.

mod common;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

use common::musicxml_core::{normalize, normalize_full};
use similar::{ChangeTag, TextDiff};

/// Nom de balise d'une ligne canonique, pour grouper les écarts par famille.
fn line_tag(line: &str) -> Option<String> {
    let reste = line.trim_start().strip_prefix('<')?;
    let reste = reste.strip_prefix('/').unwrap_or(reste);
    let fin = reste
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(reste.len());
    (fin > 0).then(|| reste[..fin].to_owned())
}

/// Compare les lignes canoniques, compte les balises modifiées et retient la
/// première ligne qui diffère (balise + texte), dans l'ordre du document.
fn compare(
    reference: &[String],
    sortie: &[String],
) -> (usize, BTreeMap<String, usize>, Option<(String, String, String)>) {
    let a: Vec<&str> = reference.iter().map(String::as_str).collect();
    let b: Vec<&str> = sortie.iter().map(String::as_str).collect();
    let texte = TextDiff::from_slices(&a, &b);
    let mut tags = BTreeMap::new();
    let mut modifiees = 0;
    let mut tag_premier = None;
    let mut ref_premiere = None;
    let mut sortie_premiere = None;
    for change in texte.iter_all_changes() {
        if change.tag() == ChangeTag::Equal {
            continue;
        }
        modifiees += 1;
        if let Some(tag) = line_tag(change.value()) {
            *tags.entry(tag).or_insert(0) += 1;
        }
        if tag_premier.is_none() {
            tag_premier = Some(line_tag(change.value()).unwrap_or_else(|| "?".to_owned()));
        }
        if change.tag() == ChangeTag::Delete && ref_premiere.is_none() {
            ref_premiere = Some(change.value().trim().to_owned());
        }
        if change.tag() == ChangeTag::Insert && sortie_premiere.is_none() {
            sortie_premiere = Some(change.value().trim().to_owned());
        }
    }
    let premier = tag_premier.map(|tag| {
        (
            tag,
            ref_premiere.unwrap_or_default(),
            sortie_premiere.unwrap_or_default(),
        )
    });
    (modifiees, tags, premier)
}

/// Échappe une ligne canonique pour le format TSV.
fn tsv(texte: &str) -> String {
    texte
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

/// Formate la première différence sous la forme `balise|référence|sortie`.
fn colonne(premier: &Option<(String, String, String)>) -> String {
    premier
        .as_ref()
        .map(|(tag, reference, sortie)| {
            format!("{}|{}|{}", tsv(tag), tsv(reference), tsv(sortie))
        })
        .unwrap_or_default()
}

#[test]
#[ignore = "outil de mesure : lancé explicitement par l'oracle MusicXML"]
fn compare_paires() {
    let Ok(manifeste) = env::var("SCORETK_ORACLE_MANIFEST") else {
        return;
    };
    let Ok(sortie) = env::var("SCORETK_ORACLE_OUTPUT") else {
        return;
    };
    let contenu = fs::read_to_string(&manifeste).expect("manifeste lisible");
    let mut resultats = String::new();
    for ligne in contenu.lines() {
        if ligne.trim().is_empty() {
            continue;
        }
        let champs: Vec<&str> = ligne.splitn(3, '\t').collect();
        assert_eq!(champs.len(), 3, "ligne de manifeste invalide : {ligne}");
        let (id, reference, sortie_xml) = (champs[0], champs[1], champs[2]);
        let ref_xml = fs::read_to_string(reference).expect("référence lisible");
        let sortie_xml = fs::read_to_string(sortie_xml).expect("sortie lisible");
        let ref_core = normalize(&ref_xml);
        let sortie_core = normalize(&sortie_xml);
        let (noyau, _, premier_noyau) = compare(&ref_core, &sortie_core);
        let (plein, tags, premier_plein) =
            compare(&normalize_full(&ref_xml), &normalize_full(&sortie_xml));
        let tags: Vec<String> = tags.iter().map(|(tag, n)| format!("{tag}:{n}")).collect();
        resultats.push_str(&format!(
            "{id}\t{noyau}\t{plein}\t{}\t{}\t{}\n",
            tags.join(","),
            colonne(&premier_noyau),
            colonne(&premier_plein),
        ));
    }
    fs::write(Path::new(&sortie), resultats).expect("résultats écrits");
}
