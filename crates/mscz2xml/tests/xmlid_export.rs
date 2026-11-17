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

//! Tests de l'option `ExportOptions::stable_ids`.
//!
//! L'export par défaut doit rester identique octet pour octet aux références
//! MuseScore 4.7.4 (aucun `id` sur `<measure>` ni `<note>`) ; l'option ajoute
//! des identifiants calculés à partir des seuls indices du modèle, donc
//! reproductibles d'un export à l'autre.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use mscz2xml::export::{export_musicxml, export_musicxml_with, ExportOptions};
use mscz2xml::model::Score;
use mscz2xml::read::read_score;
use mscz2xml::MsczContainer;

/// Fichiers du corpus utilisés : ornements, plusieurs voix, silences.
const FICHIERS: &[(&str, &str)] = &[
    ("v3", "grace"),
    ("v3", "voices"),
    ("v4", "dynamics"),
    ("v3", "moonlight"),
    ("v4", "linked-ties"),
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("racine du dépôt")
        .join("tests/corpus")
}

fn lire_score(version: &str, nom: &str) -> Score {
    let chemin = corpus_dir().join(version).join(format!("{nom}.mscx"));
    let octets = fs::read(&chemin)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", chemin.display()));
    let conteneur = MsczContainer::from_bytes(&octets)
        .unwrap_or_else(|error| panic!("conteneur {} invalide : {error}", chemin.display()));
    read_score(&conteneur)
        .unwrap_or_else(|error| panic!("lecture de {} échouée : {error}", chemin.display()))
}

fn exporter(score: &Score, stable_ids: bool) -> String {
    export_musicxml_with(
        score,
        &ExportOptions {
            stable_ids,
            ..ExportOptions::default()
        },
    )
    .expect("export MusicXML")
}

/// Mesure lue dans la sortie : identifiant et notes dans l'ordre du document.
#[derive(Debug, Default)]
struct MesureXml {
    id: Option<String>,
    xml_id: Option<String>,
    /// `(id, xml:id, est_un_silence)` par `<note>`.
    notes: Vec<(Option<String>, Option<String>, bool)>,
}

fn attribut(element: &quick_xml::events::BytesStart<'_>, nom: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribut| attribut.key.as_ref() == nom)
        .map(|attribut| attribut.value.into_owned())
}

fn parser(xml: &str) -> Vec<Vec<MesureXml>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut parties: Vec<Vec<MesureXml>> = Vec::new();
    let mut partie: Option<Vec<MesureXml>> = None;
    let mut mesure: Option<MesureXml> = None;
    let mut note: Option<(Option<String>, Option<String>, bool)> = None;
    let mut tampon = Vec::new();

    loop {
        match reader.read_event_into(&mut tampon) {
            Ok(Event::Start(element)) => match element.name().as_ref() {
                "part" => partie = Some(Vec::new()),
                "measure" => {
                    mesure = Some(MesureXml {
                        id: attribut(&element, "id"),
                        xml_id: attribut(&element, "xml:id"),
                        notes: Vec::new(),
                    });
                }
                "note" => {
                    note = Some((
                        attribut(&element, "id"),
                        attribut(&element, "xml:id"),
                        false,
                    ))
                }
                "rest" => {
                    if let Some((_, _, silence)) = note.as_mut() {
                        *silence = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(element)) => {
                if element.name().as_ref() == "rest" {
                    if let Some((_, _, silence)) = note.as_mut() {
                        *silence = true;
                    }
                }
            }
            Ok(Event::End(element)) => match element.name().as_ref() {
                "part" => {
                    if let Some(partie) = partie.take() {
                        parties.push(partie);
                    }
                }
                "measure" => {
                    if let (Some(partie), Some(mesure)) = (partie.as_mut(), mesure.take()) {
                        partie.push(mesure);
                    }
                }
                "note" => {
                    if let (Some(mesure), Some(note)) = (mesure.as_mut(), note.take()) {
                        mesure.notes.push(note);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("XML invalide : {error}"),
        }
        tampon.clear();
    }
    parties
}

/// Rang `c{chord}` d'un identifiant de note, et préfixe piste.
fn rang_de_note(id: &str) -> Option<(String, usize)> {
    let (prefixe, reste) = id.split_once("-c")?;
    let (rang, _) = reste.split_once("-n")?;
    Some((prefixe.to_owned(), rang.parse().ok()?))
}

#[test]
fn export_par_defaut_sans_ids_et_invariant() {
    for (version, nom) in FICHIERS {
        let score = lire_score(version, nom);
        let defaut = export_musicxml(&score).expect("export par défaut");
        let explicite = exporter(&score, false);
        assert_eq!(
            defaut, explicite,
            "{version}/{nom} : export par défaut modifié"
        );

        let parties = parser(&defaut);
        for (index, partie) in parties.iter().enumerate() {
            for (numero, mesure) in partie.iter().enumerate() {
                assert!(
                    mesure.id.is_none() && mesure.xml_id.is_none(),
                    "{version}/{nom} : id sur la mesure {index}:{numero} par défaut"
                );
                for (note, xml_id, _) in &mesure.notes {
                    assert!(
                        note.is_none() && xml_id.is_none(),
                        "{version}/{nom} : id de note par défaut dans {index}:{numero}"
                    );
                }
            }
        }
    }
}

#[test]
fn ids_de_mesure_et_de_note_uniques_par_partie_et_mesure() {
    let mut ids_ornement = 0usize;
    for (version, nom) in FICHIERS {
        let score = lire_score(version, nom);
        let xml = exporter(&score, true);
        let parties = parser(&xml);

        for (index_partie, partie) in parties.iter().enumerate() {
            let mut ids_mesures = BTreeSet::new();
            for (index_mesure, mesure) in partie.iter().enumerate() {
                let id = mesure
                    .id
                    .as_deref()
                    .unwrap_or_else(|| panic!("{version}/{nom} : mesure sans id"));
                assert_eq!(
                    id,
                    format!("p{index_partie}-m{index_mesure}"),
                    "{version}/{nom} : id de mesure inattendu"
                );
                assert_eq!(
                    mesure.xml_id.as_deref(),
                    Some(id),
                    "{version}/{nom} : xml:id de mesure différent de id"
                );
                assert!(
                    ids_mesures.insert(id.to_owned()),
                    "{version}/{nom} : id de mesure dupliqué : {id}"
                );

                let mut ids_notes = BTreeSet::new();
                for (index_note, (id, xml_id, _)) in mesure.notes.iter().enumerate() {
                    let id = id.as_deref().unwrap_or_else(|| {
                        panic!("{version}/{nom} : note {index_note} sans id dans {index_mesure}")
                    });
                    assert_eq!(
                        xml_id.as_deref(),
                        Some(id),
                        "{version}/{nom} : xml:id de note différent de id"
                    );
                    assert!(
                        ids_notes.insert(id.to_owned()),
                        "{version}/{nom} : id de note dupliqué : {id}"
                    );
                    if id.contains("-g") {
                        ids_ornement += 1;
                    }
                }
            }
        }
    }
    assert!(ids_ornement > 0, "aucun ornement couvert par FICHIERS");
}

#[test]
fn ids_stables_entre_deux_exports() {
    for (version, nom) in FICHIERS {
        let score = lire_score(version, nom);
        let premier = exporter(&score, true);
        let second = exporter(&score, true);
        assert_eq!(premier, second, "{version}/{nom} : ids instables");
    }
}

#[test]
fn les_silences_ne_decalent_pas_les_ids_de_note() {
    let mut paires_verifiees = 0usize;
    for (version, nom) in FICHIERS {
        let score = lire_score(version, nom);
        let xml = exporter(&score, true);
        for (pi, partie) in parser(&xml).into_iter().enumerate() {
            for (mi, mesure) in partie.into_iter().enumerate() {
                eprintln!(
                    "{version}/{nom} p{pi} m{mi}: {:?}",
                    mesure
                        .notes
                        .iter()
                        .map(|(id, _, r)| format!(
                            "{}{}",
                            if *r { "R" } else { "N" },
                            id.as_deref().unwrap_or("-")
                        ))
                        .collect::<Vec<_>>()
                );
                for fenetre in mesure.notes.windows(2) {
                    let ((Some(precedent), _, true), (Some(suivant), _, false)) =
                        (&fenetre[0], &fenetre[1])
                    else {
                        continue;
                    };
                    let Some((prefixe_precedent, rang_precedent)) = rang_de_note(precedent) else {
                        continue;
                    };
                    let Some((prefixe_suivant, rang_suivant)) = rang_de_note(suivant) else {
                        continue;
                    };
                    if prefixe_precedent != prefixe_suivant {
                        continue;
                    }
                    assert_eq!(
                        rang_suivant,
                        rang_precedent + 1,
                        "{version}/{nom} : rang décalé après un silence ({precedent} → {suivant})"
                    );
                    paires_verifiees += 1;
                }
            }
        }
    }
    assert!(
        paires_verifiees > 0,
        "aucun couple silence → note trouvé dans le corpus choisi"
    );
}
