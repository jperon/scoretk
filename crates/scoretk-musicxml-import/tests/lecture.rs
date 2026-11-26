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

use scoretk_core::model::{BarLineType, Fraction, SegmentItem};
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
fn barre_de_fin_est_un_element_du_segment() {
    let mut xml = partition_44();
    let barline = br#"<barline location="right"><bar-style>light-light</bar-style></barline>"#;
    // Insertion avant la fermeture de la mesure.
    let end = xml.windows(10).rposition(|w| w == b"</measure>").unwrap();
    xml.splice(end..end, barline.iter().copied());
    let partition = read_musicxml(&xml).expect("lecture");
    assert_eq!(partition.measures[0].end_bar_line, BarLineType::Double);
    let found = partition.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.iter_elements())
        .any(|(_, item)| matches!(item, SegmentItem::BarLine(bar) if bar.kind == BarLineType::Double));
    assert!(found, "la barre doit aussi être un élément de segment");
}

/// Deux portées d'une même partie reçoivent la barre (`MusicXmlParserPass2::barline`,
/// importmusicxmlpass2.cpp:5819-5832) ; `dashed` est l'alias C++ de `BROKEN`
/// (`engraving/types/types.h:415`).
#[test]
fn barre_dashed_sur_toutes_les_portees() {
    let document = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <staves>2</staves>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef number="1"><sign>G</sign><line>2</line></clef>
        <clef number="2"><sign>F</sign><line>4</line></clef>
      </attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><staff>1</staff><type>whole</type></note>
      <barline location="right"><bar-style>dashed</bar-style></barline>
    </measure>
  </part>
</score-partwise>"#;
    let score = read_musicxml(document).expect("lecture");
    assert_eq!(
        score.parts[0].staves.len(),
        2,
        "les deux portées sont créées"
    );
    assert_eq!(score.measures[0].end_bar_line, BarLineType::Dashed);
    let found: Vec<_> = score.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.iter_elements())
        .filter_map(|(_, item)| match item {
            SegmentItem::BarLine(bar) => Some(bar),
            _ => None,
        })
        .collect();
    assert_eq!(found.len(), 2, "une barre par portée de la partie");
    assert!(found.iter().all(|bar| bar.kind == BarLineType::Dashed));
}

/// `short` et `tick` sont des barres normales avec une portée particulière
/// (`createBarline`, importmusicxmlpass2.cpp:2055-2061), pas des subtypes.
#[test]
fn barre_short_est_normale_avec_portee_courte() {
    let mut xml = partition_44();
    let barline = br#"<barline location="right"><bar-style>short</bar-style></barline>"#;
    let end = xml.windows(10).rposition(|w| w == b"</measure>").unwrap();
    xml.splice(end..end, barline.iter().copied());
    let score = read_musicxml(&xml).expect("lecture");
    assert_eq!(score.measures[0].end_bar_line, BarLineType::Normal);
    let found = score.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.iter_elements())
        .find_map(|(_, item)| match item {
            SegmentItem::BarLine(bar) => Some(*bar),
            _ => None,
        })
        .expect("barre de fin");
    assert_eq!(found.kind, BarLineType::Normal);
    assert_eq!((found.span_from, found.span_to), (2, -2));
}

/// Extension additive : une rupture marquée sur la mesure elle-même
/// (`scoretk-own`) n'est pas déplacée sur la mesure précédente.
#[test]
fn rupture_portee_par_la_mesure() {
    let mut xml = partition_44();
    let print = br#"<print new-system="yes" scoretk-own="yes"/>"#;
    let end = xml.windows(10).rposition(|w| w == b"</measure>").unwrap();
    xml.splice(end..end, print.iter().copied());
    let score = read_musicxml(&xml).expect("lecture");
    assert!(score.measures[0].line_break);
}

/// Marqueur invisible de l'export additif : le saut de ligne final d'un
/// `work-number` survit à la lecture (le XML est rogné par `text_trimmed`).
#[test]
fn numero_doeuvre_avec_saut_final() {
    let xml = br#"<score-partwise version="4.0">
  <work><work-number>Op. 1
&#x200b;</work-number></work>
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
  </measure></part>
</score-partwise>"#;
    let score = read_musicxml(xml).expect("lecture");
    assert_eq!(score.metadata.work_number, "Op. 1\n");
}

/// `sound@tocoda` nomme la coda cible, pas le marqueur `To Coda`.
#[test]
fn marqueur_to_coda_label_depuis_le_texte() {
    let mut xml = partition_44();
    let direction = br#"<direction placement="above"><direction-type><words>To Coda</words></direction-type><sound tocoda="codab"/></direction>"#;
    let end = xml.windows(10).rposition(|w| w == b"</measure>").unwrap();
    xml.splice(end..end, direction.iter().copied());
    let score = read_musicxml(&xml).expect("lecture");
    let marks = &score.measures[0].repeat_marks;
    assert_eq!(marks.len(), 1, "{marks:?}");
    match &marks[0] {
        scoretk_core::model::RepeatMark::Marker(marker) => {
            assert_eq!(marker.kind, scoretk_core::model::MarkerType::ToCoda);
            assert_eq!(marker.label, "coda");
        }
        other => panic!("marqueur attendu : {other:?}"),
    }
}

/// XSD 4.0 : `<key>` porte l'armure écrite et `<transpose>` le décalage de
/// l'instrument ; l'armure concert se déduit de ces deux porteurs standard.
#[test]
fn armure_concert_depuis_transpose() {
    let xml = partition_44();
    let text = String::from_utf8(xml.clone())
        .unwrap()
        .replace("<fifths>0</fifths>", "<fifths>-3</fifths>")
        .replace(
            "<clef><sign>G</sign><line>2</line></clef>",
            "<clef><sign>G</sign><line>2</line></clef>\
             <transpose><diatonic>-2</diatonic><chromatic>-3</chromatic></transpose>",
        );
    let score = read_musicxml(text.as_bytes()).expect("lecture");
    let signature = score.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.iter_elements())
        .find_map(|(_, item)| match item {
            scoretk_core::model::SegmentItem::KeySig(value) => Some(value.clone()),
            _ => None,
        })
        .expect("armure");
    assert_eq!(signature.concert_key.fifths(), 0);
    assert_eq!(signature.transposed_key.fifths(), -3);
}

/// Extension additive : un marqueur `User` et sa portée sont conservés.
#[test]
fn marqueur_user_depuis_l_attribut_additif() {
    let mut xml = partition_44();
    let direction = br#"<direction><direction-type><segno/></direction-type><sound segno="segno1" scoretk-marker-kind="User" scoretk-marker-label="segno1" scoretk-marker-staff="1,2"/></direction>"#;
    let end = xml.windows(10).rposition(|w| w == b"</measure>").unwrap();
    xml.splice(end..end, direction.iter().copied());
    let score = read_musicxml(&xml).expect("lecture");
    let marks = &score.measures[0].repeat_marks;
    assert_eq!(marks.len(), 2, "{marks:?}");
    for mark in marks {
        match mark {
            scoretk_core::model::RepeatMark::Marker(marker) => {
                assert_eq!(marker.kind, scoretk_core::model::MarkerType::User);
                assert_eq!(marker.label, "segno1");
            }
            other => panic!("marqueur attendu : {other:?}"),
        }
    }
}

/// `D.S. al Coda` joue jusqu'à `coda` et reprend à `codab` (`repeat11`).
#[test]
fn saut_ds_al_coda_jusqua_coda() {
    let mut xml = partition_44();
    let direction = br#"<direction><direction-type><words>D.S. al Coda</words></direction-type><sound dalsegno="segno"/></direction>"#;
    let end = xml.windows(10).rposition(|w| w == b"</measure>").unwrap();
    xml.splice(end..end, direction.iter().copied());
    let score = read_musicxml(&xml).expect("lecture");
    match &score.measures[0].repeat_marks[0] {
        scoretk_core::model::RepeatMark::Jump(jump) => {
            assert_eq!(jump.play_until, "coda");
            assert_eq!(jump.continue_at, "codab");
        }
        other => panic!("saut attendu : {other:?}"),
    }
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
        score.engraving.style.hide_empty_staves
            && !score.engraving.style.dont_hide_staves_in_first_system,
        "le style hideEmptyStaves doit être déduit du print-object=\"no\""
    );
}
