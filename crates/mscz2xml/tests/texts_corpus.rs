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

use mscz2xml::read::read_score;
use mscz2xml::MsczContainer;

/// Élément retenu de la comparaison : nom, attributs utiles, enfants et texte.
#[derive(Debug, Default)]
struct Frag {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Frag>,
    text: String,
}

/// Fichiers du corpus v4 : ceux qui n'ont pas de texte n'ajoutent que leurs
/// `<work>`, `<credit>` et `<creator>` à la comparaison.
fn fichiers_du_corpus() -> Vec<String> {
    let mut noms: Vec<String> = fs::read_dir(corpus_dir())
        .expect("corpus v4")
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
    noms
}

/// Écarts connus, par fichier : éléments de la référence que l'export ne
/// produit pas encore (documentés dans le rapport de la tâche).
const ECARTS_CONNUS: &[(&str, &[&str])] = &[
    // Les textes de `TBox` sont exportés par MuseScore en `<words>` au début de
    // la première mesure (`tboxTextAsWords`) ; le modèle ne lit pas les cadres.
    ("repeat65", &["1,2,3,4"]),
    // « 3x » est l'`<endText>` d'un `TextLine` : les spanners textuels ne sont
    // pas modélisés.
    ("repeat65", &["3x"]),
    // Texte de `TBox` (voir ci-dessus) : exporté en `<words>` au début de la
    // première mesure par `tboxTextAsWords`.
    ("voiceswitching", &["This test goes through each measure"]),
    // Textes de `TextLine` (spanner) : le `<words>` vient de `beginText`, le
    // trait et le `<direction-type>` vide de la ligne elle-même.
    ("mmrest", &["VII"]),
    ("part-textlines", &["System", "Staff/1", "Staff/2"]),
    // Mêmes classes pour les fichiers ajoutés avec le corpus 3.x/4.x :
    // `TextLine` (« 3x ») et textes de `TBox`.
    ("repeat66", &["3x", "1,2,3,4"]),
    ("repeat68", &["no playback"]),
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4")
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
        "sound" => attribute == "tempo",
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

/// Vrai si le fragment contient un texte parmi ses descendants.
fn contient_texte(frag: &Frag) -> bool {
    matches!(frag.name.as_str(), "words" | "metronome" | "rehearsal")
        || frag.children.iter().any(contient_texte)
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
fn notre_export(nom: &str) -> Vec<String> {
    let chemin = corpus_dir().join(format!("{nom}.mscx"));
    let data = fs::read(&chemin).expect("lecture du .mscx");
    let container = MsczContainer::from_bytes(&data).expect("conteneur");
    let score = read_score(&container).expect("lecture du .mscx");
    let xml = mscz2xml::export::export_musicxml(&score).expect("export");
    extraire(&xml)
}

/// Éléments textuels de la référence MuseScore.
fn reference(nom: &str) -> Vec<String> {
    let chemin = corpus_dir().join(format!("{nom}.ref.musicxml"));
    let xml = fs::read_to_string(&chemin).expect("référence du corpus");
    extraire(&xml)
}

#[test]
fn textes_du_corpus_conformes_aux_references() {
    let mut erreurs: Vec<String> = Vec::new();

    for nom in fichiers_du_corpus() {
        let attendus = reference(&nom);
        let obtenus = notre_export(&nom);

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

        if !manquants_non_documentes.is_empty() || !restants.is_empty() {
            erreurs.push(format!(
                "{nom} :\n  manquants :\n    {}\n  en trop :\n    {}",
                manquants_non_documentes
                    .iter()
                    .map(|element| element.as_str())
                    .collect::<Vec<_>>()
                    .join("\n    "),
                restants.join("\n    ")
            ));
        }
    }

    assert!(erreurs.is_empty(), "{}", erreurs.join("\n\n"));
}
