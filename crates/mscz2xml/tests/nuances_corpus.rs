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

//! Comparaison des nuances et soufflets au corpus MuseScore 4.7.4.
//!
//! Pour chaque fichier concerné, on lit le `.mscx`, on l'exporte et on compare
//! les éléments de la famille — `<direction>` contenant `<dynamics>`,
//! `<other-dynamics>`, `<wedge>` ou `<dashes>` — à la `.ref.musicxml`, dans
//! l'ordre du document.
//!
//! Sont ignorés parce que le modèle ne les porte pas : les attributs de mise
//! en page et de police (`default-x`, `default-y`, `relative-x`, `relative-y`,
//! `spread`, `font-*`, `color`, `line-type`, `dash-length`, `space-length`,
//! `width`). Tout le reste est comparé : ordre des `<direction-type>`,
//! `placement`, `type`, `number`, `niente`, nom de nuance, texte de
//! `<other-dynamics>` et de `<words>`, `<sound dynamics>` et `<staff>`.
//!
//! Les fichiers dont un écart sémantique reste attendu sont nommés dans
//! [`ECARTS_CONNUS`], avec la raison : aucun écart n'est masqué.

use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use mscz2xml::export::export_musicxml;
use mscz2xml::read::read_score;
use mscz2xml::MsczContainer;

/// Fichiers du corpus couverts par la famille, avec leur version.
const FICHIERS: &[(&str, &str)] = &[
    ("v3", "measure-4"),
    ("v3", "moonlight"),
    ("v3", "selectionfilter1"),
    ("v3", "testHairpinDynamics"),
    ("v3", "testWedge4"),
    ("v3", "timesig-05"),
    ("v4", "dynamics"),
    ("v4", "dynamics_niente"),
    ("v4", "hairpin_dynamic_inside"),
    ("v4", "hairpin_dynamic_jump"),
    ("v4", "hairpin_simple"),
    ("v4", "hairpins_and_repeats"),
];

/// Écarts sémantiques connus, par fichier.
///
/// Vide : les seuls écarts sont les attributs de mise en page et de police
/// ignorés par la canonisation.
const ECARTS_CONNUS: &[(&str, &str)] = &[];

/// Élément XML minimal.
#[derive(Debug, Default)]
struct Noeud {
    nom: String,
    attributs: Vec<(String, String)>,
    texte: String,
    enfants: Vec<Noeud>,
}

/// Attributs conservés sur chaque élément d'une direction de la famille.
fn attributs_conserves(nom: &str) -> &'static [&'static str] {
    match nom {
        "direction" => &["placement", "system"],
        "wedge" => &["type", "niente", "number"],
        "dashes" => &["type", "number"],
        "sound" => &["dynamics"],
        _ => &[],
    }
}

/// Une direction appartient-elle à la famille ?
fn est_famille(noeud: &Noeud) -> bool {
    matches!(
        noeud.nom.as_str(),
        "dynamics" | "other-dynamics" | "wedge" | "dashes"
    ) || noeud.enfants.iter().any(est_famille)
}

/// Forme canonique : noms, attributs sémantiques et texte, sans indentation.
fn canoniser(noeud: &Noeud) -> String {
    let mut resultat = format!("<{}", noeud.nom);
    for (nom, valeur) in &noeud.attributs {
        if attributs_conserves(&noeud.nom).contains(&nom.as_str()) {
            resultat.push_str(&format!(" {nom}=\"{valeur}\""));
        }
    }
    resultat.push('>');
    if noeud.enfants.is_empty() {
        resultat.push_str(noeud.texte.trim());
    } else {
        for enfant in &noeud.enfants {
            resultat.push_str(&canoniser(enfant));
        }
    }
    resultat.push_str(&format!("</{}>", noeud.nom));
    resultat
}

/// Extrait les directions de la famille, canonisées et dans l'ordre.
fn extraire(xml: &str) -> Vec<String> {
    let racine = analyser(xml);
    let mut directions = Vec::new();
    collecter(&racine, &mut directions);
    directions
}

fn collecter(noeud: &Noeud, directions: &mut Vec<String>) {
    if noeud.nom == "direction" {
        if noeud.enfants.iter().any(est_famille) {
            directions.push(canoniser(noeud));
        }
        return;
    }
    for enfant in &noeud.enfants {
        collecter(enfant, directions);
    }
}

/// Analyse un document MusicXML en arbre minimal.
fn analyser(xml: &str) -> Noeud {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut pile: Vec<Noeud> = Vec::new();
    let mut racine = Noeud::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => pile.push(noeud(&event)),
            Ok(Event::Empty(event)) => {
                let noeud = noeud(&event);
                if let Some(parent) = pile.last_mut() {
                    parent.enfants.push(noeud);
                }
            }
            Ok(Event::End(_)) => {
                let Some(noeud) = pile.pop() else { break };
                if let Some(parent) = pile.last_mut() {
                    parent.enfants.push(noeud);
                } else {
                    racine = noeud;
                }
            }
            Ok(Event::Text(texte)) => {
                if let Some(noeud) = pile.last_mut() {
                    noeud.texte.push_str(texte.as_ref());
                }
            }
            Ok(Event::CData(texte)) => {
                if let Some(noeud) = pile.last_mut() {
                    noeud.texte.push_str(texte.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(noeud) = pile.last_mut() {
                    noeud.texte.push_str(reference.into_inner().as_ref());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML illisible : {error}"),
        }
    }
    racine
}

fn noeud(event: &quick_xml::events::BytesStart<'_>) -> Noeud {
    let nom = event.name().as_ref().to_owned();
    let attributs = event
        .attributes()
        .flatten()
        .map(|attribut| {
            (
                attribut.key.as_ref().to_owned(),
                attribut.value.as_ref().to_owned(),
            )
        })
        .collect();
    Noeud {
        nom,
        attributs,
        texte: String::new(),
        enfants: Vec::new(),
    }
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

fn lire_score(version: &str, nom: &str) -> mscz2xml::model::Score {
    let chemin = corpus_dir().join(version).join(format!("{nom}.mscx"));
    let octets = fs::read(&chemin)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", chemin.display()));
    let conteneur = MsczContainer::from_bytes(&octets)
        .unwrap_or_else(|error| panic!("conteneur {} invalide : {error}", chemin.display()));
    read_score(&conteneur)
        .unwrap_or_else(|error| panic!("lecture de {} échouée : {error}", chemin.display()))
}

#[test]
fn corpus_nuances_et_soufflets() {
    for (version, nom) in FICHIERS {
        let score = lire_score(version, nom);
        let obtenu = export_musicxml(&score).expect("export");
        let reference = fs::read_to_string(
            corpus_dir()
                .join(version)
                .join(format!("{nom}.ref.musicxml")),
        )
        .unwrap_or_else(|error| panic!("référence {version}/{nom} illisible : {error}"));

        let attendus = extraire(&reference);
        let trouves = extraire(&obtenu);

        if let Some((_, raison)) = ECARTS_CONNUS.iter().find(|(fichier, _)| *fichier == *nom) {
            eprintln!("{version}/{nom} : écart connu ({raison})");
        }

        let mut differences = Vec::new();
        for (index, (attendu, trouve)) in attendus.iter().zip(trouves.iter()).enumerate() {
            if attendu != trouve {
                differences.push(format!(
                    "{version}/{nom} direction {} :\n  attendu : {attendu}\n  obtenu  : {trouve}",
                    index + 1
                ));
            }
        }
        if attendus.len() != trouves.len() {
            differences.push(format!(
                "{version}/{nom} : {} directions attendues, {} obtenues",
                attendus.len(),
                trouves.len()
            ));
        }
        assert!(
            differences.is_empty(),
            "écarts de nuances/soufflets :\n{}",
            differences.join("\n")
        );
    }
}
