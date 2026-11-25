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

//! `<harmony>` : le texte d'un accord est la fondamentale, la qualité et la
//! basse (`Harmony::text()` dans `engraving/dom/harmony.cpp`), pas la seule
//! qualité. Réduit de `tests/corpus/v3/testHarmony6` et `testChordSymbols`.

use scoretk_core::model::Annotation;
use scoretk_musicxml_import::read_musicxml;

fn harmony_fields(xml: &[u8]) -> Vec<(String, String)> {
    let score = read_musicxml(xml).expect("lecture MusicXML");
    let mut result = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for annotation in segment.annotations_on(scoretk_core::model::Track::new(
                scoretk_core::model::StaffId::from_index(0),
                0,
            )) {
                if let Annotation::Harmony(harmony) = annotation {
                    result.push((harmony.text.clone(), harmony.name.clone()));
                }
            }
        }
    }
    result
}

fn harmonies(xml: &[u8]) -> Vec<String> {
    harmony_fields(xml)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

fn document(harmony: &str) -> Vec<u8> {
    format!(
        r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions></attributes>
    {harmony}
  </measure></part>
</score-partwise>"#
    )
    .into_bytes()
}

#[test]
fn racine_et_qualite() {
    assert_eq!(
        harmonies(&document(
            r#"<harmony><root><root-step>C</root-step></root><kind text="m">minor</kind></harmony>"#
        )),
        ["Cm"]
    );
}

#[test]
fn racine_bemol_et_basse() {
    assert_eq!(
        harmonies(&document(
            r#"<harmony><root><root-step>E</root-step><root-alter>-1</root-alter></root>
               <kind text="m">minor</kind>
               <bass><bass-step>F</bass-step></bass>
               <degree><degree-value>9</degree-value><degree-alter>0</degree-alter><degree-type>add</degree-type></degree>
               </harmony>"#
        )),
        ["Ebm(add9)/F"]
    );
}

#[test]
fn le_suffixe_analysable_reste_dans_name() {
    assert_eq!(
        harmony_fields(&document(
            r#"<harmony><root><root-step>C</root-step></root><kind text="m">minor</kind></harmony>"#
        )),
        [("Cm".to_owned(), "m".to_owned())]
    );
}

#[test]
fn harmonie_sans_fondamentale_garde_son_texte() {
    assert_eq!(
        harmonies(&document(
            r#"<harmony><root><root-step text="">C</root-step></root>
               <kind text="&lt;&gt;&amp;&quot;">none</kind></harmony>"#
        )),
        ["<>&\""]
    );
}
