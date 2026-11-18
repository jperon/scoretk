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

//! Comparaison des notations au corpus MuseScore 4.7.4.
//!
//! Pour chaque fichier concerné, on lit le `.mscx`, on l'exporte et on compare
//! les `<notations>` de la famille — articulations, points d'orgue, liaisons
//! d'expression, ornements, trilles, trémolos, techniques, arpèges,
//! glissandos, respirations et lignes d'accord — à la `.ref.musicxml`, dans
//! l'ordre du document.
//!
//! Sont ignorés parce que le modèle ne les porte pas : les attributs de mise
//! en page (`default-x`, `default-y`, `relative-x`, `relative-y`) et les
//! courbes de Bézier (`bezier-x`, `bezier-y`), qui viennent de la mise en page.
//! Tout le reste est comparé : ordre des éléments, noms, texte, attributs
//! `type`, `number`, `placement`, `orientation`, `line-type`, `direction`,
//! `slash`, `long`, `approach`, `departure`, `smufl`, et l'ouverture
//! `print-object="no"` d'un `<notations>` invisible.
//!
//! Les fichiers dont un écart sémantique reste attendu sont nommés dans
//! [`ECARTS_CONNUS`], avec les éléments manquants ou excédentaires exacts :
//! aucun écart n'est masqué.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use scoretk_musescore_import::read::read_score;
use scoretk_musescore_import::MsczContainer;
use scoretk_musicxml_export::export_musicxml;

/// Fichiers du corpus contenant des notations de la famille.
const FICHIERS: &[(&str, &str)] = &[
    ("v3", "layout_elements"),
    ("v3", "moonlight"),
    ("v3", "timesig-05"),
    ("v3", "voices"),
    ("v4", "changevisibility"),
    ("v4", "copypaste_parts"),
    ("v4", "tenuto_accent"),
    ("v4", "testExcludeInvisibleElements"),
    ("v4", "timesig-09"),
];

/// Écart attendu sur un fichier : éléments manquants et excédentaires.
struct EcartConnu {
    fichier: &'static str,
    raison: &'static str,
    manquants: &'static [&'static str],
    excedentaires: &'static [&'static str],
}

/// Écarts sémantiques connus, par fichier.
///
/// Tous les fichiers de [`FICHIERS`] sont désormais identiques à leur
/// référence : la liste reste vide et le test exige une parité exacte.
const ECARTS_CONNUS: &[EcartConnu] = &[];

/// Élément XML minimal.
#[derive(Debug, Default)]
struct Noeud {
    nom: String,
    attributs: Vec<(String, String)>,
    texte: String,
    enfants: Vec<Noeud>,
}

/// Balises de la famille comparées.
const FAMILLE: &[&str] = &[
    "articulations",
    "fermata",
    "slur",
    "ornaments",
    "technical",
    "arpeggiate",
    "non-arpeggiate",
    "glissando",
    "slide",
    "wavy-line",
    "tremolo",
    "breath-mark",
    "caesura",
    "trill-mark",
    "turn",
    "inverted-turn",
    "vertical-turn",
    "inverted-vertical-turn",
    "mordent",
    "inverted-mordent",
    "haydn",
    "schleifer",
    "fingering",
    "pluck",
    "harmonic",
    "open",
    "stopped",
    "snap-pizzicato",
    "up-bow",
    "down-bow",
    "strong-accent",
    "soft-accent",
    "stress",
    "unstress",
    "spiccato",
    "accent",
    "staccato",
    "staccatissimo",
    "tenuto",
    "detached-legato",
    "falloff",
    "doit",
    "plop",
    "scoop",
    "accidental-mark",
    "other-ornament",
    "other-articulation",
];

/// Attributs conservés sur chaque élément de la famille.
fn attributs_conserves(nom: &str) -> &'static [&'static str] {
    match nom {
        "harmonie" => &[],
        "notations" => &["print-object"],
        _ => &[
            "type",
            "number",
            "placement",
            "orientation",
            "line-type",
            "direction",
            "slash",
            "long",
            "approach",
            "departure",
            "smufl",
            "hand",
            "size",
            "bracket",
            "parentheses",
        ],
    }
}

fn est_famille(noeud: &Noeud) -> bool {
    FAMILLE.contains(&noeud.nom.as_str()) || noeud.enfants.iter().any(est_famille)
}

/// Forme canonique : noms, attributs sémantiques et texte.
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
            if FAMILLE.contains(&enfant.nom.as_str()) {
                resultat.push_str(&canoniser(enfant));
            }
        }
    }
    resultat.push_str(&format!("</{}>", noeud.nom));
    resultat
}

/// Extrait les `<notations>` de la famille, canonisés et dans l'ordre.
fn extraire(xml: &str) -> Vec<String> {
    let racine = analyser(xml);
    let mut notations = Vec::new();
    collecter(&racine, &mut notations);
    notations
}

fn collecter(noeud: &Noeud, notations: &mut Vec<String>) {
    if noeud.nom == "notations" {
        if est_famille(noeud) {
            notations.push(canoniser(noeud));
        }
        return;
    }
    for enfant in &noeud.enfants {
        collecter(enfant, notations);
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

fn lire_score(version: &str, nom: &str) -> scoretk_core::model::Score {
    let chemin = corpus_dir().join(version).join(format!("{nom}.mscx"));
    let octets = fs::read(&chemin)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", chemin.display()));
    let conteneur = MsczContainer::from_bytes(&octets)
        .unwrap_or_else(|error| panic!("conteneur {} invalide : {error}", chemin.display()));
    read_score(&conteneur)
        .unwrap_or_else(|error| panic!("lecture de {} échouée : {error}", chemin.display()))
}

/// Compare les listes d'éléments, en tenant compte des écarts déclarés.
fn comparer(nom: &str, attendus: &[String], trouves: &[String]) -> Vec<String> {
    let ecart = ECARTS_CONNUS.iter().find(|ecart| ecart.fichier == nom);

    if ecart.is_none() {
        // Fichier conforme : comparaison exacte, dans l'ordre.
        let mut differences = Vec::new();
        for (index, (attendu, trouve)) in attendus.iter().zip(trouves.iter()).enumerate() {
            if attendu != trouve {
                differences.push(format!(
                    "{nom} notations {} :\n  attendu : {attendu}\n  obtenu  : {trouve}",
                    index + 1
                ));
            }
        }
        if attendus.len() != trouves.len() {
            differences.push(format!(
                "{nom} : {} notations attendues, {} obtenues",
                attendus.len(),
                trouves.len()
            ));
        }
        return differences;
    }

    // Fichier à écart connu : le contenu doit correspondre au multiensemble
    // près, une fois retirés les éléments déclarés manquants ou excédentaires.
    let Some(ecart) = ecart else { unreachable!() };
    let mut restant: BTreeMap<&str, isize> = BTreeMap::new();
    for element in attendus {
        *restant.entry(element.as_str()).or_default() += 1;
    }
    for element in trouves {
        *restant.entry(element.as_str()).or_default() -= 1;
    }
    for manquant in ecart.manquants {
        let entry = restant.entry(manquant).or_default();
        *entry -= 1;
    }
    for excedentaire in ecart.excedentaires {
        let entry = restant.entry(excedentaire).or_default();
        *entry += 1;
    }
    let mut differences = Vec::new();
    for (element, reste) in restant {
        if reste != 0 {
            differences.push(format!(
                "{nom} : {} × {element} en écart (reste {reste})",
                reste.unsigned_abs()
            ));
        }
    }
    differences
}

#[test]
fn corpus_notations() {
    for (version, nom) in FICHIERS {
        let score = lire_score(version, nom);
        let obtenu = export_musicxml(&score).expect("export");
        let reference = fs::read_to_string(
            corpus_dir()
                .join(version)
                .join(format!("{nom}.ref.musicxml")),
        )
        .unwrap_or_else(|error| panic!("référence {version}/{nom} illisible : {error}"));

        if let Some(ecart) = ECARTS_CONNUS.iter().find(|ecart| ecart.fichier == *nom) {
            eprintln!("{version}/{nom} : écart connu ({})", ecart.raison);
        }

        let attendus = extraire(&reference);
        let trouves = extraire(&obtenu);
        let differences = comparer(nom, &attendus, &trouves);
        assert!(
            differences.is_empty(),
            "écarts de notations :\n{}",
            differences.join("\n")
        );
    }
}
