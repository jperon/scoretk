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

//! Tests de lecture minimaux, hors corpus.
//!
//! Ils couvrent les gardes de progression du lecteur : une mesure complète ne
//! doit jamais faire boucler la première passe (la correction de longueur de
//! mesure du C++ réécrit `4/4` en `8/8`, ce que la `Fraction` toujours réduite
//! du modèle ne peut pas représenter ; l'implémentation doit donc avancer).

use scoretk_core::model::{BarLineType, Fraction};
use scoretk_musicxml_import::read_musicxml;

/// Document minimal d'une mesure 4/4 avec une ronde.
fn partition_44() -> Vec<u8> {
    br#"<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Piano</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>4</duration>
        <voice>1</voice>
        <type>whole</type>
      </note>
    </measure>
  </part>
</score-partwise>"#
        .to_vec()
}

#[test]
fn mesure_44_ne_boucle_pas() {
    let score = read_musicxml(&partition_44()).expect("lecture");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::ONE);
    assert_eq!(score.measures[0].nominal_len, Fraction::ONE);
    assert_eq!(score.notes.len(), 1);
    assert_eq!(score.notes[0].pitch, 60);
}

#[test]
fn reprise_et_liaison() {
    let document = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <time><beats>4</beats><beat-type>4</beat-type></time>
      </attributes>
      <barline location="left"><repeat direction="forward"/></barline>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type>
        <tie type="start"/><notations><tied type="start"/></notations></note>
    </measure>
    <measure number="2">
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type>
        <tie type="stop"/><notations><tied type="stop"/></notations></note>
      <barline location="right"><repeat direction="backward" times="3"/></barline>
    </measure>
  </part>
</score-partwise>"#;
    let score = read_musicxml(document).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].start_bar_line, BarLineType::StartRepeat);
    assert_eq!(score.measures[1].end_bar_line, BarLineType::EndRepeat);
    assert_eq!(score.measures[1].repeat_count, 3);
    assert_eq!(score.notes.len(), 2);
    assert_eq!(
        score.notes[0].tie_forward,
        Some(scoretk_core::model::NoteId::from_index(1))
    );
    assert_eq!(
        score.notes[1].tie_back,
        Some(scoretk_core::model::NoteId::from_index(0))
    );
}

#[test]
fn racine_inconnue_refusee() {
    let error = read_musicxml(b"<score-timewise/>").expect_err("doit échouer");
    assert!(matches!(
        error,
        scoretk_musicxml_import::Error::UnknownRoot(_)
            | scoretk_musicxml_import::Error::MissingPartList
    ));
}

/// `MusicXmlParserPass2::staffDetails` (`importmusicxmlpass2.cpp:3262-3292`)
/// lit `print-object="no"` sur `<staff-details>` et rend la portée invisible.
/// Le portage ne lisait que `<staff-lines>`.
#[test]
fn staff_details_print_object_no_rend_la_portee_invisible() {
    let mut texte = String::from_utf8(partition_44()).expect("utf8");
    texte = texte.replacen(
        "<divisions>1</divisions>",
        "<divisions>1</divisions>\n        <staff-details print-object=\"no\"><staff-lines>5</staff-lines></staff-details>",
        1,
    );
    let score = read_musicxml(texte.as_bytes()).expect("lecture");
    let staff = score.parts[0].staves[0];
    assert!(
        !score.staff(staff).visible,
        "print-object=\"no\" doit rendre la portée invisible"
    );
}

/// Un `<staff-details print-object="no"/>` sur une mesure ne contenant qu'une
/// pause est une portée masquée par `hideEmptyStaves`, pas une portée
/// invisible (`importmusicxmlpass2.cpp:3275-3286`). L'export de scoretk écrit
/// ce cas pour une portée masquée sur un seul système : la rendre invisible
/// pour toute la partition était faux, et un `<staff-details>` sans attribut
/// sur la mesure suivante la rendait de nouveau visible.
#[test]
fn staff_details_portee_vide_masquee_pas_invisible() {
    let document = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <staff-details print-object="no"/>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note><rest measure="yes"/><duration>4</duration><voice>1</voice></note>
    </measure>
    <measure number="2">
      <attributes>
        <staff-details/>
      </attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note>
    </measure>
  </part>
</score-partwise>"#;
    let score = read_musicxml(document).expect("lecture");
    let staff = score.parts[0].staves[0];
    assert!(
        score.staff(staff).visible,
        "la portée masquée sur un système ne doit pas devenir invisible"
    );
    assert!(
        score.style.hide_empty_staves && !score.style.dont_hide_staves_in_first_system,
        "le style hideEmptyStaves doit être déduit du print-object=\"no\""
    );
}
