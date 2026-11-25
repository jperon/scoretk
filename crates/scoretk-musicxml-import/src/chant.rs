// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture des conventions d'interopérabilité
// grégorienne de MusicXML 4.0 (`musicxml-4.0.xsd` : `other-notation`,
// `other-direction`, `other-technical`, `notehead`, `time/senza-misura`) et de
// la syntaxe de Gregorio (`doc/Gabc.tex`, commit b63f074), réécrits en Rust
// en 2026.
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

//! Restauration de la charge grégorienne (L6) d'un document MusicXML.
//!
//! MusicXML 4.0 ne nomme pas les neumes grégoriens, mais ses éléments
//! d'extension (`other-direction`, `other-notation`, `other-technical`) portent
//! des champs structurés : en-têtes, clé, syllabes, lettres et glyphes. Le
//! lecteur les relit **champ par champ** et reconstruit la projection dérivée
//! avec `Gregorian::build_derived_score` : les durées et mesures du `.gabc`
//! n'étaient jamais des données source (`Gabc.tex:801-845`). Aucun document
//! sans ces extensions n'est modifié.

use std::collections::BTreeMap;

use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, NeumeSign, Score, SignKind,
    Syllable,
};
use scoretk_xml_common::Element;

const PREFIX: &str = "gabc:";

#[derive(Default)]
struct Collected {
    headers: Vec<GregorianHeader>,
    clef: Option<String>,
    staff_lines: u8,
    syllables: BTreeMap<usize, Syllable>,
    notes: BTreeMap<usize, Vec<GregorianNote>>,
    found: bool,
}

/// Restaure le payload grégorien et la projection dérivée quand le document
/// porte les extensions décrites dans `scoretk-musicxml-export/src/chant.rs`.
pub(crate) fn restore(root: &Element, score: &mut Score) {
    let Some(part) = root.children_named("part").next() else {
        return;
    };
    let mut collected = Collected::default();
    for (measure_index, measure) in part.children_named("measure").enumerate() {
        for direction in measure.children_named("direction") {
            for kind in direction.children_named("direction-type") {
                for other in kind.children_named("other-direction") {
                    read_direction(&other.text, &mut collected);
                }
            }
        }
        for note in measure.children_named("note") {
            for notations in note.children_named("notations") {
                for other in notations.children_named("other-notation") {
                    if let Some(note) = read_note(&other.text) {
                        collected.notes.entry(measure_index).or_default().push(note);
                        collected.found = true;
                    }
                }
                for technical in notations.children_named("technical") {
                    for other in technical.children_named("other-technical") {
                        read_sign(&other.text, &mut collected, measure_index);
                    }
                }
            }
        }
    }
    if !collected.found {
        return;
    }
    let gregorian = build_payload(collected);
    let mut rebuilt = gregorian.build_derived_score();
    rebuilt.metadata = score.metadata.clone();
    for loss in &score.report.losses {
        rebuilt.report.losses.push(loss.clone());
    }
    rebuilt.extensions.format = Some(Extension::Gregorian(gregorian));
    *score = rebuilt;
}

fn read_direction(text: &str, collected: &mut Collected) {
    let Some((kind, fields)) = decode(text) else {
        return;
    };
    match kind.as_str() {
        "header" => {
            collected.headers.push(GregorianHeader {
                name: value(&fields, 'n').unwrap_or_default().to_owned(),
                value: value(&fields, 'v').unwrap_or_default().to_owned(),
            });
            collected.found = true;
        }
        "clef" => {
            collected.clef = value(&fields, 't').map(str::to_owned);
            collected.staff_lines = value(&fields, 'l')
                .and_then(|lines| lines.parse().ok())
                .unwrap_or(4);
            collected.found = true;
        }
        "syllable" => {
            let index = value(&fields, 'i')
                .and_then(|index| index.parse().ok())
                .unwrap_or(0);
            collected.found = true;
            let syllable = collected.syllables.entry(index).or_default();
            syllable.text = value(&fields, 't').unwrap_or_default().to_owned();
            syllable.space_after = value(&fields, 'w') == Some("1");
            syllable.clef = value(&fields, 'c').map(str::to_owned);
            syllable.rubric = value(&fields, 'r').map(str::to_owned);
            for (_, bar) in fields.iter().filter(|(key, _)| *key == 'b') {
                if let Some((kind, _)) = BarKind::parse_prefix(bar) {
                    syllable.bars.push(kind);
                }
            }
            for (_, sign) in fields.iter().filter(|(key, _)| *key == 'z') {
                syllable.signs.push(sign_from_raw(sign));
            }
        }
        _ => {}
    }
}

fn read_note(text: &str) -> Option<GregorianNote> {
    let (kind, fields) = decode(text)?;
    if kind != "note" {
        return None;
    }
    Some(GregorianNote {
        letter: value(&fields, 'l')?.chars().next()?,
        glyph: value(&fields, 'g').unwrap_or_default().to_owned(),
        signs: Vec::new(),
    })
}

fn read_sign(text: &str, collected: &mut Collected, measure_index: usize) {
    let Some((kind, fields)) = decode(text) else {
        return;
    };
    if kind != "sign" {
        return;
    }
    if let Some(raw) = value(&fields, 's') {
        if let Some(note) = collected
            .notes
            .get_mut(&measure_index)
            .and_then(|notes| notes.last_mut())
        {
            note.signs.push(sign_from_raw(raw));
        }
        collected.found = true;
    }
}

fn sign_from_raw(raw: &str) -> NeumeSign {
    let kind = match raw.chars().next() {
        Some('.') => SignKind::PunctumMora,
        Some('\'') => SignKind::Ictus,
        Some('_') => SignKind::HorizontalEpisema,
        Some('o' | 'O') => SignKind::Oriscus,
        Some('w') => SignKind::Quilisma,
        Some('v' | 'V') => SignKind::Virga,
        Some('s') => SignKind::Stropha,
        Some('~' | '<' | '>') => SignKind::Liquescentia,
        Some('=') => SignKind::Linea,
        Some('r') => SignKind::Cavum,
        Some('x' | 'X' | '#' | 'y' | 'Y') => SignKind::Alteration,
        _ => SignKind::Other,
    };
    NeumeSign::new(kind, raw.to_owned())
}

fn build_payload(collected: Collected) -> Gregorian {
    let ClefState { clef, staff_lines } = ClefState::from(&collected);
    let count = collected
        .syllables
        .keys()
        .chain(collected.notes.keys())
        .copied()
        .max()
        .map_or(0, |index| index + 1);
    let mut syllables = Vec::with_capacity(count);
    for index in 0..count {
        let mut syllable = collected.syllables.get(&index).cloned().unwrap_or_default();
        if let Some(notes) = collected.notes.get(&index) {
            syllable.notes = notes.clone();
        }
        syllables.push(syllable);
    }
    Gregorian {
        headers: collected.headers,
        staff_lines,
        clefs: clef.into_iter().collect(),
        syllables,
        derived_modern_view: true,
    }
}

struct ClefState {
    clef: Option<String>,
    staff_lines: u8,
}

impl ClefState {
    fn from(collected: &Collected) -> Self {
        let clef = collected.clef.clone().or_else(|| {
            collected
                .syllables
                .values()
                .find_map(|syllable| syllable.clef.clone())
        });
        Self {
            clef,
            staff_lines: if collected.staff_lines == 0 {
                4
            } else {
                collected.staff_lines
            },
        }
    }
}

/// Décode `gabc:kind:clé=valeur|…` en conservant les `\|`, `\\`, `\n`, `\r`.
fn decode(text: &str) -> Option<(String, Vec<(char, String)>)> {
    let text = text.trim();
    let body = text.strip_prefix(PREFIX)?;
    let (kind, fields) = body.split_once(':')?;
    let mut result = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    let push_field = |current: &mut String, result: &mut Vec<(char, String)>| {
        if let Some((key, value)) = current.split_once('=') {
            if let Some(key) = key.chars().next() {
                result.push((key, value.to_owned()));
            }
        }
        current.clear();
    };
    for character in fields.chars() {
        if escaped {
            match character {
                'n' => current.push('\n'),
                'r' => current.push('\r'),
                'g' => current.push('>'),
                'l' => current.push('<'),
                other => current.push(other),
            }
            escaped = false;
        } else {
            match character {
                '\\' => escaped = true,
                '|' => push_field(&mut current, &mut result),
                other => current.push(other),
            }
        }
    }
    push_field(&mut current, &mut result);
    Some((kind.to_owned(), result))
}

fn value(fields: &[(char, String)], key: char) -> Option<&str> {
    fields
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    const XML: &str = r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions></attributes>
      <direction print-object="no"><direction-type><other-direction print-object="no">gabc:header:n=name|v=Veni</other-direction></direction-type></direction>
      <direction print-object="no"><direction-type><other-direction print-object="no">gabc:clef:t=c3|l=4</other-direction></direction-type></direction>
      <direction print-object="no"><direction-type><other-direction print-object="no">gabc:syllable:i=0|t=Ve\|ni|w=1|b=::</other-direction></direction-type></direction>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type>
        <notations print-object="no"><other-notation type="single">gabc:note:l=g|g=h_</other-notation></notations>
        <notations print-object="no"><technical><other-technical print-object="no">gabc:sign:s='</other-technical></technical></notations>
      </note>
    </measure>
  </part>
</score-partwise>"#;

    #[test]
    fn gabc_extensions_restore_the_payload_and_derived_view() {
        let score = parse_document(XML.as_bytes()).expect("a valid MusicXML document");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("the L6 payload was restored");
        };
        assert_eq!(gregorian.headers[0].name, "name");
        assert_eq!(gregorian.headers[0].value, "Veni");
        assert_eq!(gregorian.clefs, vec!["c3"]);
        assert_eq!(gregorian.staff_lines, 4);
        assert_eq!(gregorian.syllables.len(), 1);
        assert_eq!(gregorian.syllables[0].text, "Ve|ni");
        assert!(gregorian.syllables[0].space_after);
        assert_eq!(gregorian.syllables[0].bars, vec![BarKind::DivisioFinalis]);
        assert_eq!(gregorian.syllables[0].notes[0].letter, 'g');
        assert_eq!(gregorian.syllables[0].notes[0].glyph, "h_");
        assert_eq!(gregorian.syllables[0].notes[0].signs[0].raw, "'");
        assert!(gregorian.derived_modern_view);
        assert_eq!(score.measures.len(), 1);
    }

    #[test]
    fn a_document_without_gabc_extensions_is_untouched() {
        let xml = XML.replace("gabc:", "other:");
        let score = parse_document(xml.as_bytes()).expect("a valid MusicXML document");
        assert!(score.extensions.format.is_none());
    }

    #[test]
    fn escapes_are_reversible() {
        let (kind, fields) =
            decode("gabc:syllable:i=0|t=a\\|b\\\\c\\nd\\ge").expect("a field list");
        assert_eq!(kind, "syllable");
        assert_eq!(value(&fields, 't'), Some("a|b\\c\nd>e"));
    }
}
