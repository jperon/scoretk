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

//! Lecture de tout le corpus `.mscx` 3.x et comparaison avec les références
//! MusicXML produites par MuseScore.
//!
//! Pour chaque fichier `X.mscx` de `tests/corpus/v3`, on vérifie que le lecteur
//! 3.x ne renvoie pas d'erreur et que le modèle obtenu correspond à
//! `X.ref.musicxml` :
//!
//! * parties : nombre de `<score-part>` de la liste des parties ;
//! * mesures : nombre de `<measure>` (multiplié par le nombre de parties,
//!   puisque le modèle partage ses mesures entre les portées) ;
//! * notes : nombre de `<note>` qui ne sont pas des silences (`<rest/>`) ;
//! * hauteurs et orthographes : même multiensemble de hauteurs et de
//!   (`degré`, `altération`) que la référence, pour les partitions sans
//!   instrument transpositeur ;
//! * cohérence interne : mesures contiguës, aucun élément placé hors de sa
//!   mesure.
//!
//! Le corpus ne contient que des fichiers 3.00 à 3.02.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;
use scoretk_core::model::{tpc, ChordRestKind, Fraction, NoteId, Score, SegmentItem, SpannerType};
use scoretk_musescore_import::read::read302;

/// Compteurs extraits d'une référence MusicXML.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    measures: usize,
    parts: usize,
    notes: usize,
    ties: usize,
}

/// Hauteur écrite et orthographe d'une note de la référence.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ReferenceNote {
    pitch: u8,
    step: String,
    alter: i32,
}

/// Fichier du corpus et sa référence.
struct CorpusEntry {
    name: String,
    score: Score,
    counts: Counts,
    notes: Vec<ReferenceNote>,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/v3")
}

fn load_corpus() -> Vec<CorpusEntry> {
    let directory = corpus_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .expect("corpus v3 lisible")
        .map(|entry| entry.expect("entrée de répertoire lisible").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "aucun fichier .mscx dans {}",
        directory.display()
    );

    files
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .expect("nom de fichier")
                .to_string_lossy()
                .into_owned();
            let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{name} : {error}"));
            let version = scoretk_musescore_import::detect_version(&bytes)
                .unwrap_or_else(|error| panic!("{name} : version illisible : {error}"));
            let score = read302::read(&bytes, version)
                .unwrap_or_else(|error| panic!("{name} : lecture impossible : {error}"));
            let reference = directory.join(format!("{name}.ref.musicxml"));
            CorpusEntry {
                name,
                score,
                counts: count_reference(&reference),
                notes: reference_notes(&reference),
            }
        })
        .collect()
}

fn has_type(element: &quick_xml::events::BytesStart<'_>, expected: &str) -> bool {
    element
        .attributes()
        .flatten()
        .any(|attribute| attribute.key.as_ref() == "type" && attribute.value.as_ref() == expected)
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
                "tie" if has_type(&empty, "start") => counts.ties += 1,
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

/// Hauteurs écrites des notes de la référence, dans l'ordre du document.
fn reference_notes(path: &Path) -> Vec<ReferenceNote> {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{} : {error}", path.display()));
    let mut reader = Reader::from_reader(bytes.as_slice());
    let mut notes = Vec::new();
    let mut current: Option<ReferenceNote> = None;
    let mut capture = "";
    let mut has_step = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => match start.name().as_ref() {
                "note" => {
                    current = Some(ReferenceNote {
                        pitch: 0,
                        step: String::new(),
                        alter: 0,
                    });
                    has_step = false;
                }
                "step" => capture = "step",
                "alter" => capture = "alter",
                "octave" => capture = "octave",
                _ => {}
            },
            Ok(Event::Text(text)) => {
                let Some(note) = &mut current else {
                    continue;
                };
                let text = text.into_inner();
                let text = text.trim();
                match capture {
                    "step" => {
                        note.step = text.to_owned();
                        has_step = true;
                    }
                    "alter" => note.alter = text.parse().unwrap_or(0),
                    "octave" => {
                        let octave: i32 = text.parse().unwrap_or(4);
                        note.pitch = midi(&note.step, note.alter, octave);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(end)) => match end.name().as_ref() {
                "step" | "alter" | "octave" => capture = "",
                "note" => {
                    // Une note sans `<pitch>` est un silence.
                    if let Some(note) = current.take() {
                        if has_step {
                            notes.push(note);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("{} : XML invalide : {error}", path.display()),
        }
    }
    notes
}

fn midi(step: &str, alter: i32, octave: i32) -> u8 {
    let semitone = match step {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => 0,
    };
    (12 * (octave + 1) + semitone + alter).clamp(0, 127) as u8
}

/// Orthographe `(degré, altération)` d'un `tpc`.
fn spelling(value: i8) -> (String, i32) {
    if value == tpc::INVALID {
        return (String::new(), 0);
    }
    const STEPS: [&str; 7] = ["F", "C", "G", "D", "A", "E", "B"];
    // `TPC_MIN` = -8 (`Fbbb`), plus petit `tpc` valide.
    let offset = i32::from(value) + 8;
    let step = STEPS[offset.rem_euclid(7) as usize];
    let alteration = offset.div_euclid(7) - 3;
    (step.to_owned(), alteration)
}

/// Une ottava décale la hauteur *écrite* des notes qu'elle couvre.
///
/// L'export MusicXML de MuseScore applique ce décalage à `<pitch>`
/// (`ExportMusicXml::pitch2xml`, `ppitch() - pitch()`), alors que le modèle
/// garde la hauteur écrite et le spanner : les deux informations y sont, mais
/// la comparaison directe des hauteurs n'a plus de sens.
fn has_ottava(score: &Score) -> bool {
    score
        .spanners
        .iter()
        .any(|spanner| matches!(spanner.kind, SpannerType::Ottava(_)))
}

fn has_transposing_instrument(score: &Score) -> bool {
    score.parts.iter().any(|part| {
        part.instrument
            .transpose
            .is_some_and(|transpose| transpose.chromatic != 0 || transpose.diatonic != 0)
    })
}

#[test]
fn corpus_v3_se_lit_sans_erreur_et_a_les_bons_compteurs() {
    let mut differences: Vec<String> = Vec::new();

    for entry in load_corpus() {
        let measures = entry.score.measures.len() * entry.score.parts.len();
        println!(
            "{} : {} parties, {} mesures ({} × {}), {} notes ; référence : {} parties, {} mesures, {} notes",
            entry.name,
            entry.score.parts.len(),
            measures,
            entry.score.measures.len(),
            entry.score.parts.len(),
            entry.score.notes.len(),
            entry.counts.parts,
            entry.counts.measures,
            entry.counts.notes
        );

        if entry.score.parts.len() != entry.counts.parts {
            differences.push(format!(
                "{} : {} parties lues pour {} dans la référence",
                entry.name,
                entry.score.parts.len(),
                entry.counts.parts
            ));
        }
        if measures != entry.counts.measures {
            differences.push(format!(
                "{} : {measures} mesures lues pour {} dans la référence",
                entry.name, entry.counts.measures
            ));
        }
        if entry.score.notes.len() != entry.counts.notes {
            differences.push(format!(
                "{} : {} notes lues pour {} dans la référence",
                entry.name,
                entry.score.notes.len(),
                entry.counts.notes
            ));
        }
        let ties = entry
            .score
            .notes
            .iter()
            .filter(|note| note.tie_forward.is_some())
            .count();
        if ties != entry.counts.ties {
            differences.push(format!(
                "{} : {ties} liaisons commencées pour {} dans la référence",
                entry.name, entry.counts.ties
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "écarts avec les références MusicXML :\n{}",
        differences.join("\n")
    );
}

/// Les hauteurs et les orthographes doivent correspondre à la référence.
///
/// Deux cas sont écartés pour la comparaison des hauteurs :
///
/// * les partitions à instrument transpositeur : MuseScore écrit la hauteur
///   *écrite*, et le corpus change d'instrument en cours de route (la
///   transposition par tick n'est pas modélisée, seul l'instrument principal
///   l'est) ;
/// * les partitions à ottava : l'export applique le décalage d'octave à
///   `<pitch>`, le modèle le garde dans le spanner.
///
/// Les orthographes restent comparées dans les deux cas (l'ottava ne change
/// pas le degré ni l'altération).
#[test]
fn corpus_v3_hauteurs_et_orthographes() {
    let mut differences: Vec<String> = Vec::new();

    for entry in load_corpus() {
        let transposing = has_transposing_instrument(&entry.score);
        if transposing || has_ottava(&entry.score) {
            println!(
                "{} : hauteurs écrites non comparées ({})",
                entry.name,
                if transposing {
                    "instrument transpositeur"
                } else {
                    "ottava appliquée par l'export"
                }
            );
        } else {
            let mut expected_pitches: Vec<u8> = entry.notes.iter().map(|note| note.pitch).collect();
            let mut actual_pitches: Vec<u8> =
                entry.score.notes.iter().map(|note| note.pitch).collect();
            expected_pitches.sort_unstable();
            actual_pitches.sort_unstable();
            if actual_pitches != expected_pitches {
                differences.push(format!(
                    "{} : hauteurs différentes ({} notes lues)",
                    entry.name,
                    actual_pitches.len()
                ));
            }
        }

        let mut expected_spellings: Vec<(String, i32)> = entry
            .notes
            .iter()
            .map(|note| (note.step.clone(), note.alter))
            .collect();
        let mut actual_spellings: Vec<(String, i32)> = entry
            .score
            .notes
            .iter()
            .map(|note| spelling(note.tpc_transposed))
            .collect();
        expected_spellings.sort();
        actual_spellings.sort();
        if actual_spellings != expected_spellings {
            eprintln!(
                "{} : attendu {:?} / lu {:?}",
                entry.name,
                &expected_spellings[..expected_spellings.len().min(6)],
                &actual_spellings[..actual_spellings.len().min(6)]
            );
            differences.push(format!(
                "{} : orthographes différentes ({} notes lues)",
                entry.name,
                actual_spellings.len()
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "écarts de hauteur ou d'orthographe :\n{}",
        differences.join("\n")
    );
}

/// Position de chaque note (tick du segment qui la porte).
fn note_positions(score: &Score) -> HashMap<NoteId, Fraction> {
    let mut positions = HashMap::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                let SegmentItem::ChordRest(chord_rest) = item else {
                    continue;
                };
                let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                    continue;
                };
                for &note in &chord.notes {
                    positions.insert(note, segment.tick);
                }
                for grace in &chord.grace_chords {
                    for &note in &grace.chord.notes {
                        positions.insert(note, segment.tick);
                    }
                }
            }
        }
    }
    positions
}

/// Les liaisons de prolongation relient deux notes de même hauteur, plus tard.
#[test]
fn corpus_v3_liaisons_coherentes() {
    for entry in load_corpus() {
        let positions = note_positions(&entry.score);
        for (index, note) in entry.score.notes.iter().enumerate() {
            let id = NoteId::from_index(index);
            if let Some(forward) = note.tie_forward {
                let target = entry.score.note(forward);
                assert_eq!(
                    target.pitch, note.pitch,
                    "{} : liaison vers une note de hauteur différente",
                    entry.name
                );
                assert_eq!(
                    target.tie_back,
                    Some(id),
                    "{} : liaison avant/arrière non symétrique",
                    entry.name
                );
                assert!(
                    positions[&forward] > positions[&id],
                    "{} : liaison qui ne va pas vers l'avant",
                    entry.name
                );
            }
            if let Some(back) = note.tie_back {
                assert_eq!(
                    entry.score.note(back).tie_forward,
                    Some(id),
                    "{} : liaison arrière/avant non symétrique",
                    entry.name
                );
            }
        }
    }
}

/// Cohérence interne : mesures contiguës, pas de débordement, spanners orientés.
#[test]
fn corpus_v3_mesures_coherentes() {
    for entry in load_corpus() {
        for spanner in &entry.score.spanners {
            assert!(
                spanner.start.tick <= spanner.end.tick,
                "{} : spanner {:?} orienté à l'envers ({} > {})",
                entry.name,
                spanner.kind,
                spanner.start.tick.as_f64(),
                spanner.end.tick.as_f64()
            );
            assert!(
                spanner.start.tick >= Fraction::ZERO,
                "{} : spanner {:?} avant le début",
                entry.name,
                spanner.kind
            );
        }
        for (index, measure) in entry.score.measures.iter().enumerate() {
            assert!(
                measure.len.is_positive(),
                "{} : mesure {index} de durée nulle",
                entry.name
            );
            if index > 0 {
                assert_eq!(
                    measure.start_tick,
                    entry.score.measures[index - 1].end_tick(),
                    "{} : mesure {index} n'enchaîne pas sur la précédente",
                    entry.name
                );
            }
            for segment in &measure.segments {
                assert!(
                    segment.tick >= measure.start_tick && segment.tick <= measure.end_tick(),
                    "{} : segment à {} hors de la mesure {index} ({}..{})",
                    entry.name,
                    segment.tick.as_f64(),
                    measure.start_tick.as_f64(),
                    measure.end_tick().as_f64()
                );
            }
        }
    }
}

/// Même portage que pour 4.x : `TRead::readLegacyStaffName` convertit `<br/>`
/// en saut de ligne. Le lecteur 3.x doit garder l'information, sinon l'export
/// MusicXML aplatit `S\nA` en `SA`.
#[test]
fn nom_de_partie_3x_avec_saut_de_ligne() {
    let mscx = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="3.02">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <Instrument id="voice">
        <longName>S<br/>A</longName>
        <shortName>S<br/>A</shortName>
        <Channel><program value="52"/></Channel>
        </Instrument>
      </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>eighth</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
          <Chord><durationType>eighth</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
          <Rest><durationType>quarter</durationType></Rest>
          </voice>
        </Measure>
    </Staff>
  </Score>
  </museScore>
"#;
    let container = scoretk_musescore_import::MsczContainer::from_bytes(mscx.as_bytes())
        .expect("conteneur 3.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 3.x");

    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.parts[0].long_name, "S\nA");
    assert_eq!(score.parts[0].short_name, "S\nA");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.notes.len(), 3);
}
