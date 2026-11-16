//! Lecture de tout le corpus `.mscx` 3.x et comparaison avec les références
//! MusicXML produites par MuseScore.
//!
//! Pour chaque fichier `X.mscx` de `tests/corpus/v3`, on vérifie que le lecteur
//! 3.x ne renvoie pas d'erreur et que le modèle obtenu contient autant de
//! parties, de mesures et de notes que `X.ref.musicxml` :
//!
//! * parties : nombre de `<score-part>` de la liste des parties ;
//! * mesures : nombre de `<measure>` (multiplié par le nombre de parties,
//!   puisque le modèle partage ses mesures entre les portées) ;
//! * notes : nombre de `<note>` qui ne sont pas des silences (`<rest/>`).
//!
//! Le corpus ne contient que des fichiers 3.00 à 3.02.

use std::fs;
use std::path::{Path, PathBuf};

use mscz2xml::read::read302;
use quick_xml::events::Event;
use quick_xml::Reader;

/// Compteurs extraits d'une référence MusicXML.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    measures: usize,
    parts: usize,
    notes: usize,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/v3")
}

fn corpus_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("corpus v3 lisible")
        .map(|entry| entry.expect("entrée de répertoire lisible").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mscx"))
        .collect();
    files.sort();
    files
}

fn count_reference(path: &Path) -> Counts {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{} : {error}", path.display()));
    let mut reader = Reader::from_reader(bytes.as_slice());
    let mut counts = Counts::default();
    let mut depth = 0usize;
    let mut note_depth: Option<usize> = None;
    let mut note_has_rest = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                depth += 1;
                match start.name().as_ref() {
                    "measure" => counts.measures += 1,
                    "score-part" => counts.parts += 1,
                    "note" => {
                        note_depth = Some(depth);
                        note_has_rest = false;
                    }
                    "rest" if note_depth.is_some() => note_has_rest = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(empty)) => match empty.name().as_ref() {
                "measure" => counts.measures += 1,
                "score-part" => counts.parts += 1,
                "rest" if note_depth.is_some() => note_has_rest = true,
                "note" => counts.notes += 1,
                _ => {}
            },
            Ok(Event::End(_)) => {
                if note_depth == Some(depth) {
                    if !note_has_rest {
                        counts.notes += 1;
                    }
                    note_depth = None;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("{} : XML invalide : {error}", path.display()),
        }
    }
    counts
}

#[test]
fn corpus_v3_se_lit_et_correspond_aux_references() {
    let directory = corpus_dir();
    let files = corpus_files();
    assert!(
        !files.is_empty(),
        "aucun fichier .mscx dans {}",
        directory.display()
    );

    let mut differences: Vec<String> = Vec::new();

    for path in files {
        let name = path
            .file_stem()
            .expect("nom de fichier")
            .to_string_lossy()
            .into_owned();
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{name} : {error}"));
        let version = mscz2xml::detect_version(&bytes)
            .unwrap_or_else(|error| panic!("{name} : version illisible : {error}"));
        let score = read302::read(&bytes, version)
            .unwrap_or_else(|error| panic!("{name} : lecture impossible : {error}"));

        let reference = count_reference(&directory.join(format!("{name}.ref.musicxml")));
        let measures = score.measures.len() * score.parts.len();

        println!(
            "{name} (version {version}) : {} parties, {} mesures, {} notes
             référence : {} parties, {} mesures, {} notes",
            score.parts.len(),
            score.measures.len(),
            score.notes.len(),
            reference.parts,
            reference.measures,
            reference.notes
        );

        if score.parts.len() != reference.parts {
            differences.push(format!(
                "{name} : {} parties lues pour {} dans la référence",
                score.parts.len(),
                reference.parts
            ));
        }
        if measures != reference.measures {
            differences.push(format!(
                "{name} : {measures} mesures lues pour {} dans la référence",
                reference.measures
            ));
        }
        if score.notes.len() != reference.notes {
            differences.push(format!(
                "{name} : {} notes lues pour {} dans la référence",
                score.notes.len(),
                reference.notes
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "écarts avec les références MusicXML :\n{}",
        differences.join("\n")
    );
}
