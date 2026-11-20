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

//! Lecture des fichiers MuseScore 2.x (`read206`).
//!
//! Les petits fichiers `.mscx` ci-dessous sont écrits à la main : ils ne
//! proviennent d'aucune partition personnelle. Ils couvrent les écarts de
//! structure entre 2.x et 3.x identifiés dans `read206.cpp` et décrits dans
//! `docs/agents/mscx-format.md` : absence de blocs `<voice>`, voix portées par
//! `<track>`/`<tick>`, `<Tie id>`/`<endSpanner id>` et `<Tuplet id>` référencé.
//! La syntaxe est vérifiée contre `read206.cpp` (aucune spécification publiée
//! du format, voir `docs/agents/format-ref/musescore.md`).

use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem, Track};
use scoretk_musescore_import::read::read_score;
use scoretk_musescore_import::MsczContainer;

fn score(body: &str) -> Score {
    let xml = format!(
        r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Flute</trackName>
  <Instrument id="wind.flutes.flute"><longName>Flute</longName>
    <trackName>Flute</trackName><instrumentId>wind.flutes.flute</instrumentId>
    <Channel><program value="73"/></Channel></Instrument></Part>
{body}
</Score></museScore>"#
    );
    let container = MsczContainer::from_bytes(xml.as_bytes()).expect("conteneur .mscx");
    read_score(&container).expect("lecture 2.x")
}

/// Notes d'une piste d'une mesure, avec leur figure et leur tick.
fn notes(score: &Score, measure: usize, track: u32) -> Vec<(u8, Fraction, Fraction)> {
    let track = Track::from_raw(track);
    let mut segments: Vec<_> = score.measures[measure].segments.iter().collect();
    segments.sort_by_key(|segment| segment.tick);
    let mut out = Vec::new();
    for segment in segments {
        let Some(item) = segment.element(track) else {
            continue;
        };
        let SegmentItem::ChordRest(chord_rest) = item else {
            continue;
        };
        let ChordRestKind::Chord(chord) = &chord_rest.kind else {
            continue;
        };
        for &id in &chord.notes {
            out.push((
                score.note(id).pitch,
                chord_rest.duration.ticks(),
                segment.tick,
            ));
        }
    }
    out
}

#[test]
fn mesure_2x_sans_bloc_voice() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <KeySig><accidental>2</accidental></KeySig>
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Dynamic><subtype>f</subtype><text>f</text></Dynamic>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>half</durationType>
      <Lyrics><syllabic>single</syllabic><text>la</text></Lyrics>
      <Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    </Measure>
  <Measure number="2"><Rest><durationType>measure</durationType></Rest></Measure>
  </Staff>"#,
    );
    assert_eq!(score.measures.len(), 2, "deux mesures lues");
    assert_eq!(
        notes(&score, 0, 0),
        vec![
            (60, Fraction::new(1, 4), Fraction::new(0, 1)),
            (64, Fraction::new(1, 2), Fraction::new(1, 4)),
            (62, Fraction::new(1, 4), Fraction::new(3, 4)),
        ],
        "hauteurs, figures et ticks des accords 2.x"
    );
    assert!(
        score.measures[0].segments.iter().any(|segment| {
            segment
                .annotations_on(Track::from_raw(0))
                .any(|annotation| matches!(annotation, scoretk_core::model::Annotation::Dynamic(_)))
        }),
        "une nuance 2.x est lue"
    );
}

#[test]
fn voix_et_portees_depuis_les_tracks() {
    let xml = r#"<museScore version="2.06"><Score>
<Division>480</Division>
<Part><Staff id="1"/><Staff id="2"/><trackName>Piano</trackName>
  <Instrument><longName>Piano</longName><Channel><program value="0"/></Channel></Instrument></Part>
<Staff id="1"><Measure number="1">
  <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
  </Measure></Staff>
<Staff id="2"><Measure number="1">
  <Chord><durationType>half</durationType><Note><pitch>48</pitch><tpc>14</tpc></Note></Chord>
  <tick>960</tick>
  <Chord><track>5</track><durationType>half</durationType>
    <Note><track>5</track><pitch>55</pitch><tpc>18</tpc></Note></Chord>
  </Measure></Staff>
</Score></museScore>"#;
    let container = MsczContainer::from_bytes(xml.as_bytes()).expect("conteneur .mscx");
    let score = read_score(&container).expect("lecture 2.x");
    assert_eq!(score.staves.len(), 2, "deux portées déclarées");
    assert_eq!(
        notes(&score, 0, 0),
        vec![(60, Fraction::new(1, 1), Fraction::ZERO)]
    );
    assert_eq!(
        notes(&score, 0, 4),
        vec![(48, Fraction::new(1, 2), Fraction::ZERO)]
    );
    assert_eq!(
        notes(&score, 0, 5),
        vec![(55, Fraction::new(1, 2), Fraction::new(1, 2))],
        "la seconde voix de la portée 2 est placée à son tick absolu 2.x"
    );
}

#[test]
fn n_olets_et_liaisons_2x() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Tuplet id="1"><normalNotes>2</normalNotes><actualNotes>3</actualNotes>
      <baseNote>eighth</baseNote><Number><text>3</text></Number></Tuplet>
    <Chord><Tuplet>1</Tuplet><durationType>eighth</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc><Tie id="7"/></Note></Chord>
    <Chord><Tuplet>1</Tuplet><durationType>eighth</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    <Chord><Tuplet>1</Tuplet><durationType>eighth</durationType>
      <Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
    </Measure>
  <Measure number="2">
    <Chord><durationType>quarter</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc><endSpanner id="7"/></Note></Chord>
    </Measure>
  </Staff>"#,
    );
    assert_eq!(score.tuplets.len(), 1, "un n-olet défini par id");
    assert_eq!(score.tuplets[0].ratio, Fraction::new(3, 2));
    assert_eq!(
        notes(&score, 0, 0),
        vec![
            (60, Fraction::new(1, 8), Fraction::ZERO),
            (62, Fraction::new(1, 8), Fraction::new(1, 12)),
            (64, Fraction::new(1, 8), Fraction::new(1, 6)),
            (72, Fraction::new(1, 4), Fraction::new(1, 4)),
        ],
        "les trois notes du n-olet gardent leur figure, la suivante n'hérite pas du ratio"
    );
    let first = score.measures[0]
        .segments
        .iter()
        .find_map(|segment| {
            segment
                .element(Track::from_raw(0))
                .and_then(|item| item.as_chord_rest())
                .and_then(|chord_rest| chord_rest.chord_ref())
        })
        .and_then(|chord| chord.notes.first().copied())
        .expect("première note");
    let last = score.measures[1]
        .segments
        .iter()
        .find_map(|segment| {
            segment
                .element(Track::from_raw(0))
                .and_then(|item| item.as_chord_rest())
                .and_then(|chord_rest| chord_rest.chord_ref())
        })
        .and_then(|chord| chord.notes.first().copied())
        .expect("note finale");
    assert_eq!(score.note(first).tie_forward, Some(last), "liaison avant");
    assert_eq!(score.note(last).tie_back, Some(first), "liaison arrière");
}

#[test]
fn ottava_2x_appariee_par_id() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <Ottava id="78"><subtype>8va</subtype></Ottava>
    <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></Measure>
  <Measure number="2"><endSpanner id="78"/>
    <Chord><durationType>whole</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></Measure>
  </Staff>"#,
    );
    let ottavas: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, scoretk_core::model::SpannerType::Ottava(_)))
        .collect();
    assert_eq!(ottavas.len(), 1, "une ottava reconstruite");
    assert_eq!(
        ottavas[0].kind,
        scoretk_core::model::SpannerType::Ottava(scoretk_core::model::OttavaType::OctaveUp)
    );
    assert_eq!(ottavas[0].start.tick, score.measures[0].start_tick);
    assert_eq!(ottavas[0].end.tick, score.measures[1].start_tick);
}

#[test]
fn hairpin_2x_apparie_par_id() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <HairPin id="12"><subtype>0</subtype>
      <beginText><text>cresc.</text></beginText></HairPin>
    <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></Measure>
  <Measure number="2">
    <Chord><durationType>half</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc><endSpanner id="12"/></Note></Chord>
    </Measure>
  </Staff>"#,
    );
    let hairpins: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, scoretk_core::model::SpannerType::Hairpin(_)))
        .collect();
    assert_eq!(hairpins.len(), 1, "un soufflet reconstruit");
    assert_eq!(
        hairpins[0].kind,
        scoretk_core::model::SpannerType::Hairpin(scoretk_core::model::HairpinType::Crescendo)
    );
    assert_eq!(hairpins[0].hairpin.as_ref().unwrap().begin_text, "cresc.");
    assert_eq!(hairpins[0].start.tick, score.measures[0].start_tick);
    assert_eq!(hairpins[0].end.tick, score.measures[1].start_tick);
}

#[test]
fn slur_2x_apparie_par_id() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <Slur id="5"><track>0</track><up>up</up></Slur>
    <Chord><durationType>quarter</durationType>
      <Slur type="start" id="5"/><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Slur type="stop" id="5"/><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>65</pitch><tpc>19</tpc></Note></Chord>
    </Measure>
  </Staff>"#,
    );
    let slurs: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| spanner.kind == scoretk_core::model::SpannerType::Slur)
        .collect();
    assert_eq!(slurs.len(), 1, "une seule liaison reconstruite");
    assert_eq!(slurs[0].start.tick, Fraction::new(0, 1));
    assert_eq!(slurs[0].end.tick, Fraction::new(1, 2));
    let placement = slurs[0]
        .notation
        .as_ref()
        .and_then(|notation| match notation {
            scoretk_core::model::SpannerNotation::Slur(slur) => slur.placement,
            _ => None,
        });
    assert_eq!(placement, Some(scoretk_core::model::Placement::Above));
}

#[test]
fn volta_2x_appariee_par_id() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1"><startRepeat/>
    <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></Measure>
  <Measure number="2">
    <Volta id="9"><endHook>1</endHook>
      <beginText><text>1.</text></beginText><endings>1</endings></Volta>
    <Chord><durationType>whole</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></Measure>
  <Measure number="3"><endSpanner id="9"/><endRepeat>2</endRepeat>
    <Chord><durationType>whole</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord></Measure>
  </Staff>"#,
    );
    let voltas: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| spanner.kind == scoretk_core::model::SpannerType::Volta)
        .collect();
    assert_eq!(voltas.len(), 1, "une seule volta reconstruite");
    let info = voltas[0].volta.as_ref().expect("informations de volta");
    assert_eq!(info.endings, vec![1]);
    assert!(info.closed, "endHook ferme la volta");
    assert_eq!(voltas[0].text.as_deref(), Some("1."));
    assert_eq!(voltas[0].start.tick, score.measures[1].start_tick);
    assert_eq!(voltas[0].end.tick, score.measures[2].start_tick);
}

#[test]
fn version_3_non_concernee() {
    let xml = r#"<museScore version="3.02"><Score><Division>480</Division>
<Part><Staff id="1"/><trackName>Flute</trackName><Instrument><Channel><program value="73"/></Channel></Instrument></Part>
<Staff id="1"><Measure number="1"><voice>
  <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
  </voice></Measure></Staff>
</Score></museScore>"#;
    let container = MsczContainer::from_bytes(xml.as_bytes()).expect("conteneur .mscx");
    let score = read_score(&container).expect("lecture 3.x");
    assert_eq!(
        notes(&score, 0, 0),
        vec![(60, Fraction::new(1, 4), Fraction::ZERO)]
    );
}

#[test]
fn alteration_explicite_2x() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <KeySig><accidental>1</accidental></KeySig>
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>quarter</durationType>
      <Note><Accidental><subtype>sharp</subtype></Accidental>
        <pitch>61</pitch><tpc>23</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><Accidental><subtype>natural</subtype></Accidental>
        <pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><Accidental><subtype>flat</subtype></Accidental>
        <pitch>63</pitch><tpc>19</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    </Measure>
  </Staff>"#,
    );
    let notes: Vec<_> = score.measures[0]
        .segments
        .iter()
        .filter_map(|segment| {
            segment
                .element(Track::from_raw(0))
                .and_then(|item| item.as_chord_rest())
                .and_then(|chord_rest| chord_rest.chord_ref())
        })
        .filter_map(|chord| chord.notes.first().copied())
        .map(|id| score.note(id))
        .collect();
    assert_eq!(notes.len(), 4);
    assert_eq!(
        notes[0].accidental.as_ref().map(|a| a.kind),
        Some(scoretk_core::model::AccidentalType::Sharp)
    );
    assert_eq!(
        notes[1].accidental.as_ref().map(|a| a.kind),
        Some(scoretk_core::model::AccidentalType::Natural)
    );
    assert_eq!(
        notes[2].accidental.as_ref().map(|a| a.kind),
        Some(scoretk_core::model::AccidentalType::Flat)
    );
    assert!(notes[3].accidental.is_none(), "sans altération explicite");
}

#[test]
fn liaison_2x_entre_deux_accords_separeres() {
    let score = score(
        r#"<Staff id="1">
  <Measure number="1">
    <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>quarter</durationType>
      <Note><Tie id="9"/><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>quarter</durationType>
      <Note><endSpanner id="9"/><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    <Chord><durationType>half</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    </Measure>
  </Staff>"#,
    );
    let notes: Vec<_> = score.measures[0]
        .segments
        .iter()
        .filter_map(|segment| {
            segment
                .element(Track::from_raw(0))
                .and_then(|item| item.as_chord_rest())
                .and_then(|chord_rest| chord_rest.chord_ref())
        })
        .filter_map(|chord| chord.notes.first().copied())
        .map(|id| score.note(id))
        .collect();
    assert_eq!(notes.len(), 3);
    assert!(notes[0].tie_forward.is_some(), "la première note doit partir en liaison");
    assert!(notes[1].tie_back.is_some(), "la deuxième note doit recevoir la liaison");
    assert!(notes[2].tie_forward.is_none(), "la troisième note est étrangère à la liaison");
}

#[test]
fn irregular_et_len_sont_distincts() {
    let score = score(
        r#"<Staff id="1">
  <Measure len="2/4">
    <TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
    <Chord><durationType>half</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
    </Measure>
  <Measure len="2/4">
    <irregular>1</irregular>
    <Chord><durationType>half</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
    </Measure>
  </Staff>"#,
    );
    assert!(
        !score.measures[0].exclude_from_numbering,
        "un simple @len ne retire pas le numéro"
    );
    assert!(
        score.measures[1].exclude_from_numbering,
        "<irregular>1</irregular> retire le numéro"
    );
}
