// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiimporter.cpp` et
// `meiconverter.cpp` de MuseScore Studio 4.7.4, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Lecture de MEI 5.1 (`mei-basic`) vers le modèle pivot.
//!
//! `MeiImporter::read` cherche le premier `./music//score`
//! (`meiimporter.cpp:1011`) puis lit `scoreDef`, les `section`s et leurs
//! mesures. Les éléments inconnus sont ignorés et comptés dans
//! `Score::report` : comme MuseScore, l'import est un analyseur « best
//! effort » et non un validateur RelaxNG.

mod barlines;
mod capabilities;
mod controls;
mod header;
mod reader;

pub use capabilities::CAPABILITIES;
pub use scoretk_mei_common::Error;

use scoretk_core::model::Score;
use scoretk_xml_common::Element;

/// Lit un document MEI en mémoire et renvoie le modèle pivot.
pub fn read_mei(bytes: &[u8]) -> Result<Score, Error> {
    let root = scoretk_xml_common::parse(bytes)?;
    if root.name != "mei" {
        return Err(Error::Unsupported(format!(
            "racine « {} » (attendu mei)",
            root.name
        )));
    }
    reader::Reader::read(&root)
}

/// Premier descendant portant ce nom, dans l'ordre du document.
pub(crate) fn first_descendant<'a>(node: &'a Element, name: &str) -> Option<&'a Element> {
    if node.name == name {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| first_descendant(child, name))
}

/// Texte direct d'un élément, ou texte du premier enfant texte/rend.
pub(crate) fn lines_of(node: &Element) -> String {
    let mut lines = Vec::new();
    if !node.text_trimmed().is_empty() {
        lines.push(node.text_trimmed().to_owned());
    }
    for child in &node.children {
        match child.name.as_str() {
            "lb" => lines.push(String::new()),
            "rend" | "hi" => {
                let text = lines_of(child);
                if !text.is_empty() {
                    lines.push(text);
                }
            }
            _ => {}
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_non_mei_root() {
        assert!(read_mei(b"<score-partwise/>").is_err());
    }

    #[test]
    fn reads_a_minimal_score() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4">
              <clef shape="G" line="2"/></staffDef></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <note dur="4" pname="c" oct="4"/><rest dur="4"/>
            </layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("minimal MEI");
        assert_eq!(score.staves.len(), 1);
        assert_eq!(score.measures.len(), 1);
        assert_eq!(score.notes.len(), 1);
        assert_eq!(score.notes[0].pitch, 60);
    }

    #[test]
    fn tempo_bpm_becomes_beats_per_second() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5"/></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <note dur="4" pname="c" oct="4"/>
            </layer></staff>
            <tempo staff="1" tstamp="1" midi.bpm="120"><rend>note</rend> = 120</tempo>
            </measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with a tempo");
        let tempo = score
            .measures
            .iter()
            .flat_map(|measure| measure.segments.iter())
            .flat_map(|segment| segment.annotations.iter())
            .find_map(|annotation| match &annotation.annotation {
                scoretk_core::model::Annotation::Tempo(tempo) => Some(tempo),
                _ => None,
            })
            .expect("tempo annotation");
        assert_eq!(tempo.beats_per_second, Some(2.0));
        assert!(tempo.text.plain().ends_with("= 120"));
    }

    #[test]
    fn reads_page_geometry_and_text_sizes_from_score_def() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef page.width="8.50000in" page.height="11.00000in"
                      page.leftmar="0.39370in" page.rightmar="0.39370in"
                      page.topmar="0.39370in" page.botmar="0.78740in"
                      vu.height="1.76400mm" optimize="true">
              <pgHead func="first">
                <rend halign="center" valign="top">
                  <rend type="title" fontsize="24pt">Title</rend>
                </rend>
              </pgHead>
              <staffGrp><staffDef n="1" lines="5"/></staffGrp>
            </scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <note dur="4" pname="c" oct="4"/>
            </layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with page geometry");
        let style = &score.engraving.style;
        assert!((style.page_width_in - 8.5).abs() < 1e-6);
        assert!((style.page_height_in - 11.0).abs() < 1e-6);
        assert!((style.page_even_left_margin_in - 0.3937).abs() < 1e-5);
        assert!((style.page_odd_bottom_margin_in - 0.7874).abs() < 1e-5);
        assert!((style.spatium_mm - 1.764).abs() < 1e-6);
        assert!(style.hide_empty_staves);
        assert_eq!(style.title_font_size, 24.0);
    }

    /// MuseScore 4.7.4 place un `<scoreDef>` de changement de mesure à
    /// l'intérieur de l'<ending> qui contient la mesure (`writeEnding`,
    /// meiexporter.cpp:795) ; `readSectionElements` doit le voir, sinon la
    /// mesure suivante est decalee d'un quart de temps.
    #[test]
    fn reads_a_score_def_inside_an_ending() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4"/></staffGrp></scoreDef>
            <section>
              <measure n="1"><staff n="1"><layer n="1"><note dur="4" pname="c" oct="4"/></layer></staff></measure>
              <ending n="1">
                <scoreDef meter.count="3" meter.unit="4"/>
                <measure n="2"><staff n="1"><layer n="1"><note dur="4" pname="d" oct="4"/></layer></staff></measure>
              </ending>
              <measure n="3"><staff n="1"><layer n="1"><note dur="4" pname="e" oct="4"/></layer></staff></measure>
            </section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with an ending");
        assert_eq!(score.measures.len(), 3);
        assert_eq!(
            score.measures[0].len,
            scoretk_core::model::Fraction::new(4, 4)
        );
        assert_eq!(
            score.measures[1].len,
            scoretk_core::model::Fraction::new(3, 4)
        );
        assert_eq!(
            score.measures[2].start_tick,
            scoretk_core::model::Fraction::new(7, 4)
        );
    }

    /// `Measure::measureToMEI` (meiconverter.cpp:2254) ne trace qu'une mesure
    /// pour un multi-repos ; `@n` saute les mesures couvertes. Le lecteur doit
    /// les recréer avec un silence entier (comme le modèle `.mscx`).
    #[test]
    fn expands_a_multi_measure_rest_span() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4"/></staffGrp></scoreDef>
            <section>
              <measure n="1"><staff n="1"><layer n="1"><mRest/></layer></staff></measure>
              <measure n="4" metcon="false" right="end"><staff n="1"><layer n="1"/></staff></measure>
            </section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with a multi-measure rest");
        // `@n="4"` après `@n="1"` : le groupe couvre les mesures 2, 3 et 4.
        assert_eq!(score.measures.len(), 4);
        for (index, measure) in score.measures.iter().enumerate() {
            assert_eq!(measure.len, scoretk_core::model::Fraction::new(4, 4));
            assert_eq!(
                measure.start_tick,
                scoretk_core::model::Fraction::from_integer(index as i64)
            );
            let rest = measure
                .segments
                .iter()
                .filter_map(|segment| {
                    segment
                        .element(scoretk_core::model::Track::new(
                            scoretk_core::model::StaffId::from_index(0),
                            0,
                        ))
                        .and_then(|item| item.as_chord_rest())
                })
                .next()
                .expect("a full-measure rest");
            assert!(rest.is_full_measure_rest());
        }
        assert_eq!(
            score.measures[3].end_bar_line,
            scoretk_core::model::BarLineType::End
        );
    }

    /// MEI 5.1 `att.keySig.log` ne porte que `@sig`/`@keysig` (nombre de
    /// quintes). `Convert::keyFromMEI` (meiconverter.cpp:2066) construit une
    /// `engraving::Key`, sans mode : le lecteur laisse `KeyMode::Unknown`.
    #[test]
    fn keysig_from_mei_has_no_mode() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" keysig="2s" meter.count="4" meter.unit="4"/></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1"><note dur="4" pname="c" oct="4"/></layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with a key signature");
        let key = score.measures[0]
            .segments
            .iter()
            .find_map(|segment| {
                segment.iter_elements().find_map(|(_, item)| match item {
                    scoretk_core::model::SegmentItem::KeySig(key) => Some(key),
                    _ => None,
                })
            })
            .expect("key signature");
        assert_eq!(key.concert_key.fifths(), 2);
        assert_eq!(key.mode, scoretk_core::model::KeyMode::Unknown);
    }

    /// MEI 5.1 `att.noteHeads` : les têtes sans terme `data.HEADSHAPE` sont
    /// nommées par un glyphe SMuFL (`@glyph.auth="smufl"`).
    #[test]
    fn reads_a_smufl_note_head() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5"/></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <note dur="4" pname="c" oct="4" glyph.auth="smufl" glyph.name="noteDoBlack"/>
            </layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with a SMuFL note head");
        assert_eq!(
            score.notes[0].head_group,
            scoretk_core::model::NoteHeadGroup::Do
        );
    }

    /// Un `<scoreDef keysig>` en cours de partition s'applique à toutes les
    /// portées (MEI 5.1 `att.scoreDef.log`), pas seulement au `scoreDef` initial.
    #[test]
    fn reads_mid_score_keysig_change() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4"/></staffGrp></scoreDef>
            <section>
              <measure n="1"><staff n="1"><layer n="1"><note dur="4" pname="c" oct="4"/></layer></staff></measure>
              <scoreDef keysig="2s"/>
              <measure n="2"><staff n="1"><layer n="1"><note dur="4" pname="d" oct="4"/></layer></staff></measure>
            </section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with a mid-score key change");
        let key = score.measures[1]
            .segments
            .iter()
            .find_map(|segment| {
                segment.iter_elements().find_map(|(_, item)| match item {
                    scoretk_core::model::SegmentItem::KeySig(key) => Some(key),
                    _ => None,
                })
            })
            .expect("changed key signature");
        assert_eq!(key.concert_key.fifths(), 2);
        assert_eq!(key.mode, scoretk_core::model::KeyMode::Unknown);
    }

    /// Les n-olets imbriqués sont écrits par MuseScore 4.7.4
    /// (`Convert::tupletToMEI`) : l'avance est divisée par le produit des
    /// ratios et l'événement appartient au n-olet le plus interne.
    #[test]
    fn reads_nested_tuplets_with_stacked_ratios() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="2" meter.unit="4"/></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <tuplet num="3" numbase="2" bracket.place="above" num.place="above">
                <note dur="4" pname="c" oct="5"/><note dur="4" pname="b" oct="4"/>
                <tuplet num="5" numbase="4">
                  <note dur="16" pname="a" oct="4"/><note dur="16" pname="g" oct="4"/>
                  <note dur="16" pname="f" oct="4"/><note dur="16" pname="e" oct="4"/>
                  <note dur="16" pname="d" oct="4"/>
                </tuplet>
              </tuplet>
            </layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let score = read_mei(bytes).expect("MEI with a nested tuplet");
        use scoretk_core::model::Fraction;
        assert_eq!(score.tuplets.len(), 2);
        assert_eq!(score.tuplets[0].len, Fraction::new(1, 2));
        assert_eq!(score.tuplets[1].len, Fraction::new(1, 4));
        assert!(score.tuplets[0].bracket);
        assert!(!score.tuplets[1].bracket);
        assert!(score.tuplets[1].parent.is_some());
        let member = score.measures[0]
            .segments
            .iter()
            .flat_map(|segment| segment.iter_elements())
            .find_map(|(_, item)| item.as_chord_rest())
            .expect("chord rest");
        assert_eq!(
            member.tuplet,
            Some(scoretk_core::model::TupletId::from_index(0))
        );
        let ticks: Vec<_> = score.measures[0]
            .segments
            .iter()
            .filter(|segment| {
                segment
                    .iter_elements()
                    .any(|(_, item)| item.as_chord_rest().is_some())
            })
            .map(|segment| segment.tick)
            .collect();
        assert_eq!(ticks[1], Fraction::new(1, 6));
        assert_eq!(ticks[2], Fraction::new(1, 3));
        assert_eq!(ticks[3], Fraction::new(11, 30));
    }
}
