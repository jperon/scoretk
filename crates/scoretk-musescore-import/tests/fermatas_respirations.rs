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

//! Lecture des points d'orgue (`<Fermata>`) et des respirations (`<Breath>`).
//!
//! Les respirations suivent `SymNames::symIdByName`
//! (`engraving/types/symnames.cpp:798-822`) : `<symbol>` porte le nom du
//! `SymId` (`caesuraCurved`, `breathMarkTick`, `breathMarkUpbow`,
//! `breathMarkSalzedo`, `chantCaesura`…), `<subtype>` numérique n'étant que
//! l'ancien format (`rw/read400/tread.cpp:2171-2196`). Le placement vient de
//! `MeasureRead::readVoice` (voix 0 au-dessus, voix 1 au-dessous), puis de
//! `<placement>` s'il est écrit (`rw/read410/measureread.cpp:323-329`).
//!
//! Les points d'orgue suivent `TRead::read(Fermata*)` : `<subtype>` porte la
//! forme (`fermata`, `fermataShort`, `fermataLong`, `fermataVeryShort`,
//! `fermataVeryLong`, `fermataLongHenze`, `fermataShortHenze`, `curlewSign`).

use scoretk_core::model::{
    Annotation, BreathType, FermataType, Placement, SegmentItem, SegmentType, StaffId, Track,
};
use scoretk_musescore_import::read::read_score;
use scoretk_musescore_import::MsczContainer;

/// `.mscx` minimal d'une partie de piano, avec le contenu de `<voice>` fourni.
fn mscx(version: &str, voice: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="{version}">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
      </Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName>
        <shortName>Pno.</shortName>
        <trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
{voice}
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
}

fn rest(duration: &str) -> String {
    format!("          <Rest><durationType>{duration}</durationType></Rest>\n")
}

fn breath(body: &str) -> String {
    format!("          <Breath>{body}</Breath>\n")
}

fn lire(version: &str, voice: &str) -> scoretk_core::model::Score {
    let source = mscx(version, voice);
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    read_score(&container).expect("lecture du .mscx")
}

fn respirations(score: &scoretk_core::model::Score) -> Vec<(BreathType, Placement)> {
    let staff = StaffId::from_raw(0);
    score.measures[0]
        .segments_of_type(SegmentType::Breath)
        .filter_map(|segment| match segment.element(Track::new(staff, 0)) {
            Some(SegmentItem::Breath(breath)) => Some((breath.kind, breath.placement)),
            _ => None,
        })
        .collect()
}

/// Les quatre respirations non-césures et deux césures, avec le placement
/// déduit de la voix puis forcé par `<placement>`.
#[test]
fn lit_les_respirations_en_3x() {
    let voice = format!(
        "{}{}{}{}{}{}{}",
        rest("quarter"),
        breath("<symbol>caesuraCurved</symbol>"),
        rest("quarter"),
        breath("<symbol>breathMarkTick</symbol>"),
        rest("quarter"),
        breath("<symbol>breathMarkUpbow</symbol>"),
        rest("half"),
    );
    assert_eq!(
        respirations(&lire("3.02", &voice)),
        vec![
            (BreathType::CaesuraCurved, Placement::Above),
            (BreathType::Tick, Placement::Above),
            (BreathType::Upbow, Placement::Above),
        ]
    );

    // `<subtype>` obsolète : 2 = césure courbe, 3 = césure.
    let voice = format!(
        "{}{}{}{}{}",
        rest("quarter"),
        breath("<subtype>2</subtype>"),
        rest("quarter"),
        breath("<subtype>3</subtype>"),
        rest("half"),
    );
    assert_eq!(
        respirations(&lire("3.02", &voice)),
        vec![
            (BreathType::CaesuraCurved, Placement::Above),
            (BreathType::Caesura, Placement::Above),
        ]
    );

    // `<placement>` explicite l'emporte sur le placement déduit de la voix ;
    // les variantes restantes sont reconnues.
    let voice = format!(
        "{}{}{}{}{}",
        rest("quarter"),
        breath("<symbol>breathMarkSalzedo</symbol><placement>below</placement>"),
        rest("quarter"),
        breath("<symbol>caesuraThick</symbol>"),
        rest("half"),
    );
    assert_eq!(
        respirations(&lire("3.02", &voice)),
        vec![
            (BreathType::Salzedo, Placement::Below),
            (BreathType::CaesuraThick, Placement::Above),
        ]
    );
}

/// Mêmes cas en 4.x, plus `chantCaesura` et `caesuraSingleStroke`, qui
/// partagent la même sortie MusicXML (`caesura single`).
#[test]
fn lit_les_respirations_en_4x() {
    let voice = format!(
        "{}{}{}{}{}{}{}{}",
        rest("eighth"),
        breath("<symbol>caesura</symbol>"),
        rest("eighth"),
        breath("<symbol>caesuraShort</symbol>"),
        rest("eighth"),
        breath("<symbol>caesuraSingleStroke</symbol>"),
        rest("eighth"),
        breath("<symbol>chantCaesura</symbol>"),
    );
    assert_eq!(
        respirations(&lire("4.70", &voice)),
        vec![
            (BreathType::Caesura, Placement::Above),
            (BreathType::CaesuraShort, Placement::Above),
            (BreathType::CaesuraSingle, Placement::Above),
            (BreathType::CaesuraSingle, Placement::Above),
        ]
    );

    let voice = format!(
        "{}{}{}{}{}{}{}",
        rest("quarter"),
        breath("<symbol>breathMarkTick</symbol>"),
        rest("quarter"),
        breath("<symbol>breathMarkUpbow</symbol><placement>below</placement>"),
        rest("quarter"),
        breath("<symbol>breathMarkSalzedo</symbol>"),
        rest("quarter"),
    );
    assert_eq!(
        respirations(&lire("4.70", &voice)),
        vec![
            (BreathType::Tick, Placement::Above),
            (BreathType::Upbow, Placement::Below),
            (BreathType::Salzedo, Placement::Above),
        ]
    );
}

fn fermata(score: &scoretk_core::model::Score) -> (FermataType, Placement, bool) {
    let staff = StaffId::from_raw(0);
    let track = Track::new(staff, 0);
    for segment in &score.measures[0].segments {
        for annotation in segment.annotations_on(track) {
            if let Annotation::Fermata(fermata) = annotation {
                return (fermata.kind, fermata.placement, fermata.visible);
            }
        }
    }
    eprintln!(
        "TESTSUMMARY measures={} staves={} notes={}",
        score.measures.len(),
        score.staves.len(),
        score.notes.len()
    );
    for (index, measure) in score.measures.iter().enumerate() {
        eprintln!(
            "measure {index} @{:?} len {:?}",
            measure.start_tick, measure.len
        );
        for segment in &measure.segments {
            eprintln!("  seg {:?} {:?}", segment.kind, segment.annotations.len());
            for entry in &segment.annotations {
                eprintln!("    {:?} -> {:?}", entry.track, entry.annotation);
            }
        }
    }
    panic!("aucun point d'orgue");
}

/// `<Fermata>` est une annotation de segment dans les deux formats, lue par
/// `MeasureRead::readVoice` (`rw/read410/measureread.cpp:534-539`), jamais
/// comme fils d'accord.
#[test]
fn lit_les_points_d_orgue_en_3x_et_en_4x() {
    let voice = format!(
        "          <Fermata><subtype>fermataLong</subtype><placement>below</placement></Fermata>\n{}",
        rest("whole")
    );
    assert_eq!(
        fermata(&lire("3.02", &voice)),
        (FermataType::Long, Placement::Below, true)
    );

    let voice = format!(
        "          <Fermata><subtype>fermataVeryShort</subtype><placement>below</placement></Fermata>\n{}",
        rest("whole")
    );
    assert_eq!(
        fermata(&lire("4.70", &voice)),
        (FermataType::VeryShort, Placement::Below, true)
    );
}

/// Un point d'orgue invisible garde `visible = false` et sa forme.
#[test]
fn lit_un_point_d_orgue_invisible() {
    let voice = format!(
        "          <Fermata><subtype>curlewSign</subtype><visible>0</visible></Fermata>\n{}",
        rest("whole")
    );
    assert_eq!(
        fermata(&lire("4.70", &voice)),
        (FermataType::Curlew, Placement::Above, false)
    );
}
