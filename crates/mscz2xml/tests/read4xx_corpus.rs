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

//! Test d'intégration du lecteur 4.x sur le corpus MuseScore.
//!
//! Pour chaque `X.mscx` de `tests/corpus/v4`, le lecteur doit produire une
//! partition sans erreur, et les nombres de mesures, de parties et de notes
//! doivent correspondre à la référence `X.ref.musicxml` exportée par MuseScore.
//!
//! Conventions de comptage, identiques à celles de l'export :
//! - une `<score-part>` par partie MuseScore ;
//! - une `<measure>` par mesure **et par partie** (MusicXML est « partwise »),
//!   d'où la division par le nombre de parties ;
//! - une `<note>` sans enfant `<rest/>` par note sonore, ornements compris.
//!   Les silences MusicXML sont eux aussi des `<note><rest/></note>` et ne
//!   comptent donc pas.

use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use mscz2xml::model::{Annotation, CreditStyle, Placement, RehearsalFrame};
use mscz2xml::read::read_score;
use mscz2xml::MsczContainer;

/// Nombres extraits d'une référence MusicXML.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    measures: usize,
    parts: usize,
    notes: usize,
}

impl Counts {
    fn read(path: &Path) -> Self {
        let data = fs::read(path).unwrap_or_else(|error| {
            panic!("lecture de {} impossible : {error}", path.display());
        });
        let mut reader = Reader::from_reader(data.as_slice());
        reader.config_mut().trim_text(true);

        let mut counts = Counts::default();
        let mut inside_note = false;
        let mut note_has_rest = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(event)) => match event.name().as_ref() {
                    "measure" => counts.measures += 1,
                    "score-part" => counts.parts += 1,
                    "note" => {
                        inside_note = true;
                        note_has_rest = false;
                    }
                    "rest" if inside_note => note_has_rest = true,
                    _ => {}
                },
                Ok(Event::Empty(event)) => match event.name().as_ref() {
                    // Une note vide n'existe pas dans MusicXML, mais un
                    // `<rest/>` auto-fermé doit être vu.
                    "rest" if inside_note => note_has_rest = true,
                    _ => {}
                },
                Ok(Event::End(event)) => {
                    if event.name().as_ref() == "note" {
                        if !note_has_rest {
                            counts.notes += 1;
                        }
                        inside_note = false;
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(error) => panic!("référence {} illisible : {error}", path.display()),
            }
        }

        counts
    }
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4")
}

#[test]
fn lit_tout_le_corpus_v4_sans_erreur() {
    let directory = corpus_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("corpus introuvable dans {} : {error}", directory.display()))
        .map(|entry| entry.expect("entrée de répertoire").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();

    assert!(!files.is_empty(), "aucun fichier .mscx dans le corpus v4");

    for path in files {
        let data = fs::read(&path).expect("lecture du .mscx");
        let container = MsczContainer::from_bytes(&data)
            .unwrap_or_else(|error| panic!("conteneur {} invalide : {error}", path.display()));

        let score = read_score(&container).unwrap_or_else(|error| {
            panic!("lecture de {} échouée : {error}", path.display());
        });

        assert!(
            !score.parts.is_empty(),
            "{} : aucune partie lue",
            path.display()
        );
        assert!(
            !score.measures.is_empty(),
            "{} : aucune mesure lue",
            path.display()
        );
    }
}

#[test]
fn nombres_de_mesures_parties_et_notes_conformes_aux_references() {
    let directory = corpus_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("corpus introuvable dans {} : {error}", directory.display()))
        .map(|entry| entry.expect("entrée de répertoire").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .collect();
    files.sort();

    let mut failures = Vec::new();

    for path in &files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("nom de fichier UTF-8");
        let reference_path =
            path.with_file_name(format!("{}.ref.musicxml", name.trim_end_matches(".mscx")));
        let reference = Counts::read(&reference_path);

        let data = fs::read(path).expect("lecture du .mscx");
        let container = MsczContainer::from_bytes(&data).expect("conteneur");
        let score = read_score(&container).expect("lecture du .mscx");

        if reference.parts == 0 || !reference.measures.is_multiple_of(reference.parts) {
            failures.push(format!(
                "{name} : référence incohérente ({} mesures pour {} parties)",
                reference.measures, reference.parts
            ));
            continue;
        }

        let expected_measures = reference.measures / reference.parts;
        if score.parts.len() != reference.parts {
            failures.push(format!(
                "{name} : {} parties lues, {} attendues",
                score.parts.len(),
                reference.parts
            ));
        }
        if score.measures.len() != expected_measures {
            failures.push(format!(
                "{name} : {} mesures lues, {expected_measures} attendues",
                score.measures.len()
            ));
        }
        if score.notes.len() != reference.notes {
            failures.push(format!(
                "{name} : {} notes lues, {} attendues",
                score.notes.len(),
                reference.notes
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "écarts avec les références MusicXML :\n{}",
        failures.join("\n")
    );
}

/// Le corpus ne contient pas de fichier 5.00 : ce test synthétique vérifie la
/// disposition propre à 5.0 (`<Part id>`, `<Staff>` sans `id`, noms dans
/// `<InstrumentLabel>`) et la lecture d'un n-plet imbriqué avec liaison.
#[test]
fn lit_un_mscx_5_00_synthetique() {
    let mscx = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="5.00">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <Instrument id="piano">
        <InstrumentLabel>
          <longName>Piano</longName>
          <shortName>Pno.</shortName>
        </InstrumentLabel>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff>
      <Measure>
        <voice>
          <TimeSig><sigN>2</sigN><sigD>4</sigD></TimeSig>
          <Tuplet><normalNotes>2</normalNotes><actualNotes>3</actualNotes><baseNote>eighth</baseNote></Tuplet>
          <Chord><durationType>eighth</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc>
              <Spanner type="Tie"><Tie/><next><location><fractions>1/12</fractions></location></next></Spanner>
            </Note>
          </Chord>
          <Chord><durationType>eighth</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc>
              <Spanner type="Tie"><Tie/><prev><location><fractions>-1/12</fractions></location></prev></Spanner>
            </Note>
          </Chord>
          <Chord><durationType>eighth</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
          <endTuplet/>
          <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#;

    let container = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur 5.00");
    let score = read_score(&container).expect("lecture 5.00");

    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.parts[0].long_name, "Piano");
    assert_eq!(score.parts[0].short_name, "Pno.");
    assert_eq!(score.parts[0].instrument.id, "keyboard.piano");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.notes.len(), 4);
    assert_eq!(score.tuplets.len(), 1);
    assert_eq!(score.tuplets[0].ratio.to_string(), "3/2");
    assert_eq!(
        score.notes[0].tie_forward,
        Some(mscz2xml::model::NoteId::from_index(1))
    );
    assert_eq!(
        score.notes[1].tie_back,
        Some(mscz2xml::model::NoteId::from_index(0))
    );
}

fn lire_du_corpus(name: &str) -> mscz2xml::model::Score {
    let path = corpus_dir().join(name);
    let data =
        fs::read(&path).unwrap_or_else(|error| panic!("lecture de {} : {error}", path.display()));
    let container = MsczContainer::from_bytes(&data).expect("conteneur");
    read_score(&container).expect("lecture du .mscx")
}

#[test]
fn liaisons_tuplets_et_paroles() {
    // `linked-ties` : trois liaisons de prolongation, dont deux impliquant des
    // ornements (appoggiature et grace8after).
    let ties = lire_du_corpus("linked-ties.mscx");
    let mut forward = 0;
    for (index, note) in ties.notes.iter().enumerate() {
        if let Some(target) = note.tie_forward {
            forward += 1;
            assert_eq!(
                ties.note(target).tie_back,
                Some(mscz2xml::model::NoteId::from_index(index)),
                "liaison {index} mal appariée"
            );
        }
    }
    assert_eq!(forward, 3);
    assert_eq!(
        ties.notes
            .iter()
            .filter(|note| note.tie_back.is_some())
            .count(),
        3
    );

    // `voiceswitching` : quinze n-olets, dont des n-olets imbriqués sur trois
    // niveaux ; les accords intérieurs pointent vers le n-olet le plus interne.
    let tuplets = lire_du_corpus("voiceswitching.mscx");
    assert_eq!(tuplets.tuplets.len(), 15);
    assert!(tuplets.tuplets.iter().any(|tuplet| tuplet.parent.is_some()));
    assert_eq!(tuplets.tuplets[0].ratio.to_string(), "3/2");
    assert_eq!(tuplets.tuplets[0].base_len.to_string(), "1/2");
    assert_eq!(tuplets.tuplets[0].len.to_string(), "1/1");
    let mut chords_in_tuplets = 0;
    for measure in &tuplets.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let mscz2xml::model::SegmentItem::ChordRest(chord_rest) = item {
                    if chord_rest.tuplet.is_some() {
                        chords_in_tuplets += 1;
                    }
                }
            }
        }
    }
    assert!(chords_in_tuplets > 0);

    // `copypastesymbollist-lyrics` : paroles sur l'accord, syllabisation et
    // mélisme lus depuis `<ticks_f>`.
    let lyrics = lire_du_corpus("copypastesymbollist-lyrics.mscx");
    let mut syllables = Vec::new();
    let mut melisma = None;
    for measure in &lyrics.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let mscz2xml::model::SegmentItem::ChordRest(chord_rest) = item {
                    for lyric in &chord_rest.lyrics {
                        syllables.push((lyric.text.clone(), lyric.syllabic, lyric.number()));
                        melisma = melisma.or(lyric.melisma);
                    }
                }
            }
        }
    }
    assert_eq!(
        syllables.first().map(|(text, _, _)| text.as_str()),
        Some("Nel")
    );
    assert_eq!(
        syllables.get(1),
        Some(&("mez".to_owned(), mscz2xml::model::LyricsSyllabic::Begin, 1))
    );
    assert_eq!(
        syllables.get(2).map(|(_, syllabic, _)| *syllabic),
        Some(mscz2xml::model::LyricsSyllabic::End)
    );
    assert_eq!(
        melisma.map(|value| value.to_string()),
        Some("1/4".to_owned())
    );
}

/// Les textes (portée, système, tempo, repère) et les crédits de la page de
/// titre sont lus avec leur balisage, leur placement, leur visibilité et leur
/// style.
#[test]
fn lit_les_textes_et_les_credits() {
    let mscx = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.00">
  <Score>
    <Division>480</Division>
    <metaTag name="workTitle">Oeuvre</metaTag>
    <Part>
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="piano">
        <longName>Piano</longName>
        <shortName>Pno.</shortName>
        <instrumentId>keyboard.piano</instrumentId>
      </Instrument>
    </Part>
    <Staff id="1">
      <VBox>
        <height>10</height>
        <Text><style>title</style><text>Mon titre</text></Text>
        <Text><style>subtitle</style><text>Sous-titre</text></Text>
        <Text><style>composer</style><text>Compositrice</text></Text>
        <Text><style>lyricist</style><visible>0</visible><text>Caché</text></Text>
      </VBox>
      <Measure>
        <voice>
          <StaffText>
            <placement>below</placement>
            <text>doux <b>et</b> <i>lié</i></text>
            </StaffText>
          <StaffText>
            <visible>0</visible>
            <text>invisible</text>
            </StaffText>
          <SystemText>
            <text>Chœur</text>
            </SystemText>
          <RehearsalMark>
            <frameType>2</frameType>
            <text><b>A</b></text>
            </RehearsalMark>
          <Expression>
            <text>dolce</text>
            </Expression>
          <InstrumentChange>
            <text>pizz.</text>
            </InstrumentChange>
          <Tempo>
            <tempo>2</tempo>
            <followText>1</followText>
            <text><sym>metNoteQuarterUp</sym> = 120</text>
            </Tempo>
          <Chord><durationType>whole</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          </voice>
        </Measure>
      </Staff>
  </Score>
</museScore>
"#;

    let container = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let score = read_score(&container).expect("lecture");

    // Crédits : les textes de `<VBox>` dans l'ordre, le texte invisible écarté.
    assert_eq!(score.credits.len(), 3);
    assert_eq!(score.credits[0].style, CreditStyle::Title);
    assert_eq!(score.credits[0].text.plain(), "Mon titre");
    assert_eq!(score.credits[0].style.credit_type(), Some("title"));
    assert_eq!(score.credits[1].style, CreditStyle::Subtitle);
    assert_eq!(score.credits[1].text.plain(), "Sous-titre");
    assert_eq!(score.credits[2].style, CreditStyle::Composer);
    assert_eq!(score.credits[2].text.plain(), "Compositrice");
    assert_eq!(score.metadata.work_title, "Oeuvre");

    let annotations = &score.measures[0].segments[0].annotations;
    assert_eq!(annotations.len(), 7);

    let Annotation::StaffText(text) = &annotations[0].annotation else {
        panic!("texte de portée attendu");
    };
    assert_eq!(text.placement, Placement::Below);
    assert_eq!(text.text.plain(), "doux et lié");
    // Quatre fragments : normal, gras, normal (l'espace après `</b>`), italique.
    assert_eq!(text.text.fragments().len(), 4);
    assert!(text.text.fragments()[1].format.bold);
    assert!(text.text.fragments()[3].format.italic);

    let Annotation::StaffText(invisible) = &annotations[1].annotation else {
        panic!("texte de portée attendu");
    };
    assert!(!invisible.visible);

    let Annotation::SystemText(system) = &annotations[2].annotation else {
        panic!("texte système attendu");
    };
    assert_eq!(system.text.plain(), "Chœur");

    let Annotation::RehearsalMark(mark) = &annotations[3].annotation else {
        panic!("repère attendu");
    };
    assert_eq!(mark.frame, RehearsalFrame::Circle);
    assert_eq!(mark.text.plain(), "A");

    assert!(matches!(
        &annotations[4].annotation,
        Annotation::Expression(expression) if expression.text.plain() == "dolce"
    ));
    assert!(matches!(
        &annotations[5].annotation,
        Annotation::InstrumentChange(change) if change.text.plain() == "pizz."
    ));

    let Annotation::Tempo(tempo) = &annotations[6].annotation else {
        panic!("tempo attendu");
    };
    assert!(tempo.follow_text);
    assert_eq!(tempo.beats_per_second, Some(2.0));
    // Le symbole de métronome garde son point de code MuseScore.
    assert_eq!(tempo.text.plain(), "\u{ECA5} = 120");
}
