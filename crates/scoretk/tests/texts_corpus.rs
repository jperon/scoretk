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

//! Comparaison des textes exportés au corpus MuseScore 4.7.
//!
//! Pour chaque fichier du corpus qui contient des textes (`StaffText`,
//! `SystemText`, `Tempo`, `RehearsalMark`) et des crédits de page de titre
//! (`VBox`), on relit le `.mscx`, on l'exporte et on compare à la référence
//! `.ref.musicxml` les seuls éléments qui relèvent des textes :
//!
//! - `<direction>` qui porte `<words>`, `<metronome>` ou `<rehearsal>`, avec
//!   ses `<offset>`, `<staff>` et `<sound tempo>` ;
//! - `<credit>` et ses `<credit-type>`/`<credit-words>` ;
//! - `<work>`, `<movement-number>`, `<movement-title>` et les `<creator>`
//!   d'`<identification>`.
//!
//! Les attributs de mise en page (`default-x`, `default-y`, `relative-x`,
//! `relative-y`, `width`, `print-object`, `color`) sont ignorés : le modèle ne
//! porte ni géométrie ni style. Tout ce qui n'est pas un texte (notes,
//! attributs de mesure, nuances, spanners) est ignoré aussi.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use scoretk_musescore_import::read::read_score;
use scoretk_musescore_import::MsczContainer;

/// Élément retenu de la comparaison : nom, attributs utiles, enfants et texte.
#[derive(Debug, Default)]
struct Frag {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Frag>,
    text: String,
}

/// Fichiers du corpus v3 et v4, avec leur répertoire. Ceux qui n'ont pas de
/// texte n'ajoutent que leurs `<work>`, `<credit>` et `<creator>` à la
/// comparaison.
fn fichiers_du_corpus() -> Vec<(String, PathBuf)> {
    let mut fichiers: Vec<(String, PathBuf)> = Vec::new();
    for version in ["v3", "v4"] {
        let mut noms: Vec<String> = fs::read_dir(corpus_dir().join(version))
            .unwrap_or_else(|_| panic!("corpus {version}"))
            .map(|entree| entree.expect("entrée de répertoire").path())
            .filter(|chemin| {
                chemin
                    .extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .map(|chemin| {
                chemin
                    .file_stem()
                    .expect("nom de fichier")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        noms.sort();
        for nom in noms {
            fichiers.push((nom, corpus_dir().join(version)));
        }
    }
    fichiers
}

/// Écarts connus, par fichier : éléments de la référence que l'export ne
/// produit pas encore (documentés dans le rapport de la tâche).
const ECARTS_CONNUS: &[(&str, &[&str])] = &[
    // `layout_elements` contient une reprise de mesure (`RepeatMeasure`) qui
    // rejoue les annotations de la mesure répétée : MuseScore ré-exporte le
    // repère « A » à la mesure répétée. C'est un écart de la famille des
    // reprises de mesure, pas des textes.
    (
        "layout_elements",
        &["<rehearsal font-size=\"14\" font-weight=\"bold\">A</rehearsal>"],
    ),
    // Les `<words>` et `<dashes>` ci-dessous sont les textes de début/fin des
    // soufflets (`HairPin`), pas des `TextLine` : leur mise en forme relève de
    // la famille des nuances (`writeHairpinText`). Ils restent listés ici pour
    // que la comparaison des textes ne les masque pas.
    (
        "testHairpinDynamics",
        &[
            "cresc. from ",
            " to ",
            "to ",
            "from ",
            "begin",
            "end",
            "dashes number=\"1\" type=\"start\"",
        ],
    ),
    ("voices", &["cresc."]),
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
}

/// Attributs conservés (les autres sont de la mise en page).
fn attribut_utile(name: &str, attribute: &str) -> bool {
    match name {
        "direction" => matches!(attribute, "placement" | "system"),
        // Pour les directions, `justify`/`valign` viennent de
        // `positioningAttributes` (mise en page) ; pour les crédits, ils font
        // partie du texte exporté.
        "words" | "rehearsal" => matches!(
            attribute,
            "font-weight"
                | "font-style"
                | "underline"
                | "line-through"
                | "font-family"
                | "font-size"
                | "enclosure"
        ),
        "credit-words" => matches!(
            attribute,
            "font-weight"
                | "font-style"
                | "underline"
                | "line-through"
                | "font-family"
                | "font-size"
                | "justify"
                | "valign"
        ),
        "metronome" => matches!(attribute, "parentheses"),
        "bracket" => matches!(
            attribute,
            "type" | "number" | "line-end" | "line-type" | "end-length"
        ),
        "dashes" => matches!(attribute, "type" | "number"),
        "sound" => attribute == "tempo",
        "instrument-change" => attribute == "id",
        "offset" => attribute == "sound",
        "credit" => attribute == "page",
        "creator" => attribute == "type",
        _ => false,
    }
}

/// Éléments conservés dans la sortie canonique.
fn element_utile(name: &str) -> bool {
    matches!(
        name,
        "direction"
            | "direction-type"
            | "words"
            | "rehearsal"
            | "metronome"
            | "beat-unit"
            | "beat-unit-dot"
            | "per-minute"
            | "bracket"
            | "dashes"
            | "swing"
            | "straight"
            | "first"
            | "second"
            | "swing-type"
            | "print"
            | "part-name-display"
            | "part-abbreviation-display"
            | "display-text"
            | "accidental-text"
            | "instrument-change"
            | "instrument-sound"
            | "sound"
            | "offset"
            | "staff"
            | "credit"
            | "credit-type"
            | "credit-words"
            | "work"
            | "work-title"
            | "work-number"
            | "movement-number"
            | "movement-title"
            | "creator"
    )
}

/// Sérialise un fragment retenu, attributs triés et mise en page écartée.
fn canonique(frag: &Frag) -> String {
    let mut attrs: Vec<(String, String)> = frag
        .attrs
        .iter()
        .filter(|(name, _)| attribut_utile(&frag.name, name))
        .cloned()
        .collect();
    attrs.sort();

    let mut out = String::new();
    out.push('<');
    out.push_str(&frag.name);
    for (name, value) in attrs {
        out.push(' ');
        out.push_str(&name);
        out.push_str("=\"");
        out.push_str(&value);
        out.push('"');
    }
    out.push('>');
    out.push_str(&frag.text);
    for child in &frag.children {
        if element_utile(&child.name) {
            out.push_str(&canonique(child));
        }
    }
    out.push_str("</");
    out.push_str(&frag.name);
    out.push('>');
    out
}

/// Vrai si le fragment contient un élément de la famille textes parmi ses
/// descendants (texte, ligne de texte, changement d'instrument, swing).
fn contient_texte(frag: &Frag) -> bool {
    matches!(
        frag.name.as_str(),
        "words"
            | "metronome"
            | "rehearsal"
            | "bracket"
            | "dashes"
            | "swing"
            | "straight"
            | "instrument-change"
    ) || frag.children.iter().any(contient_texte)
}

/// Vrai si le fragment est un `<print>` de changement d'instrument.
fn print_de_changement(frag: &Frag) -> bool {
    frag.children.iter().any(|child| {
        matches!(
            child.name.as_str(),
            "part-name-display" | "part-abbreviation-display"
        )
    })
}

/// Extrait les éléments textuels d'un document MusicXML.
fn extraire(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);

    let mut stack: Vec<Frag> = Vec::new();
    let mut racine = Frag::default();
    let mut captures: Vec<String> = Vec::new();

    let mut push = |frag: Frag, stack: &mut Vec<Frag>, captures: &mut Vec<String>| {
        let retenu = match frag.name.as_str() {
            "direction" => contient_texte(&frag),
            "print" => print_de_changement(&frag),
            "sound" => contient_texte(&frag),
            "credit" | "work" | "movement-number" | "movement-title" | "creator" => true,
            _ => false,
        };
        if retenu {
            captures.push(canonique(&frag));
        }
        match stack.last_mut() {
            Some(parent) => parent.children.push(frag),
            None => racine.children.push(frag),
        }
    };

    /// Construit un fragment à partir d'une balise ouvrante.
    fn fragment(start: &BytesStart<'_>) -> Frag {
        Frag {
            name: start.name().as_ref().to_owned(),
            attrs: start
                .attributes()
                .flatten()
                .map(|attribute| {
                    (
                        attribute.key.as_ref().to_owned(),
                        attribute.value.to_string(),
                    )
                })
                .collect(),
            children: Vec::new(),
            text: String::new(),
        }
    }

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => stack.push(fragment(&start)),
            Ok(Event::Empty(start)) => {
                push(fragment(&start), &mut stack, &mut captures);
            }
            Ok(Event::End(_)) => {
                if let Some(frag) = stack.pop() {
                    push(frag, &mut stack, &mut captures);
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(current) = stack.last_mut() {
                    current
                        .text
                        .push_str(&text.xml_content(quick_xml::XmlVersion::Implicit1_0));
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push('&');
                    current.text.push_str(reference.as_ref());
                    current.text.push(';');
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("MusicXML illisible : {error}"),
        }
    }

    captures
}

/// Éléments textuels d'une partition du corpus, lus puis exportés.
fn notre_export(nom: &str, dir: &Path) -> Vec<String> {
    let chemin = dir.join(format!("{nom}.mscx"));
    let data = fs::read(&chemin).expect("lecture du .mscx");
    let container = MsczContainer::from_bytes(&data).expect("conteneur");
    let score = read_score(&container).expect("lecture du .mscx");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    extraire(&xml)
}

/// Éléments textuels de la référence MuseScore.
fn reference(nom: &str, dir: &Path) -> Vec<String> {
    let chemin = dir.join(format!("{nom}.ref.musicxml"));
    let xml = fs::read_to_string(&chemin).expect("référence du corpus");
    extraire(&xml)
}

#[test]
fn textes_du_corpus_conformes_aux_references() {
    let mut erreurs: Vec<String> = Vec::new();

    for (nom, dir) in fichiers_du_corpus() {
        let attendus = reference(&nom, &dir);
        let obtenus = notre_export(&nom, &dir);

        let mut manquants = BTreeSet::new();
        let mut restants: Vec<String> = obtenus.clone();
        for attendu in &attendus {
            if let Some(index) = restants.iter().position(|obtenu| obtenu == attendu) {
                restants.remove(index);
            } else {
                manquants.insert(attendu.clone());
            }
        }

        // Écarts documentés : on ne les compte pas comme des régressions.
        let connus: Vec<&str> = ECARTS_CONNUS
            .iter()
            .filter(|(fichier, _)| *fichier == nom)
            .flat_map(|(_, motifs)| motifs.iter().copied())
            .collect();
        let mut manquants_non_documentes: Vec<&String> = Vec::new();
        for manquant in &manquants {
            if !connus.iter().any(|motif| manquant.contains(motif)) {
                manquants_non_documentes.push(manquant);
            }
        }
        // Les écarts documentés peuvent aussi laisser un élément « en trop »
        // (par exemple le `<words>` non formaté d'un soufflet, dont la
        // référence porte la police de la famille des nuances).
        let restants_non_documentes: Vec<&String> = restants
            .iter()
            .filter(|restant| !connus.iter().any(|motif| restant.contains(motif)))
            .collect();

        if !manquants_non_documentes.is_empty() || !restants_non_documentes.is_empty() {
            erreurs.push(format!(
                "{nom} :\n  manquants :\n    {}\n  en trop :\n    {}",
                manquants_non_documentes
                    .iter()
                    .map(|element| element.as_str())
                    .collect::<Vec<_>>()
                    .join("\n    "),
                restants_non_documentes
                    .iter()
                    .map(|element| element.as_str())
                    .collect::<Vec<_>>()
                    .join("\n    ")
            ));
        }
    }

    assert!(erreurs.is_empty(), "{}", erreurs.join("\n\n"));
}
