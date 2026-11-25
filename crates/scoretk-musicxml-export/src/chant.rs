// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : il porte les conventions d'interopérabilité
// grégorienne de MusicXML 4.0 (`musicxml-4.0.xsd` : `other-notation`,
// `other-direction`, `other-technical`, `notehead`, `time/senza-misura`) et
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

//! Transport des faits grégoriens que MusicXML peut porter.
//!
//! Un `.gabc` n'a ni mesure métrique ni durée fixe (`Gabc.tex:801-845`), mais
//! ses faits **structurés** — en-têtes, clé, syllabes, lettres de hauteur,
//! glyphes de neume, signes et barres de séparation — ont un equivalent
//! MusicXML : `<lyric>` (`syllabic`), `<barline>`, `<clef>`, `<notehead>` pour
//! les formes que l'énumération couvre, puis
//! `<other-notation>`/`<other-direction>`/`<other-technical>` pour ce que la
//! norme n'énumère pas (XSD 4.0 : « extended representation, though without
//! application interoperability »). Chaque champ est écrit **séparément** :
//! aucune source GABC brute n'est copiée dans le XML (voir le test
//! `no_raw_gabc_source_in_xml`).
//!
//! Ce qui ne peut pas être exprimé est déclaré hors de portée dans
//! `docs/agents/gabc.md` §10 : custos et sauts de ligne (`Gabc.tex:872-906`),
//! attributs TeX entre crochets (`:689-700,908-931`), coupures NABC
//! (`gabc-score-determination.y:870-907`) et ordre exact des barres à
//! l'intérieur d'un groupe.

use scoretk_core::model::{Gregorian, GregorianNote, SignKind};

use crate::exportmusicxml::Out;

/// Préfixe des champs grégoriens dans un élément d'extension MusicXML.
///
/// Le préfixe permet à un lecteur tiers d'ignorer ces données sans confondre
/// un texte grégorien avec une indication d'exécution ordinaire.
pub(crate) const PREFIX: &str = "gabc:";

/// Échappe les séparateurs et les retours à la ligne d'un champ structuré.
pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            // Certains lecteurs XML (quick-xml 0.42) perdent `&gt;` dans le
            // texte : on encode donc `>` comme une séquence propre au champ,
            // sans changer l'échappement global de l'écrivain (qui doit rester
            // identique aux références MuseScore 4.7.4).
            '>' => out.push_str("\\g"),
            '<' => out.push_str("\\l"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// Structuré `clé=valeur|clé=valeur` d'un champ d'extension.
pub(crate) fn field(key: char, value: &str) -> String {
    format!("{key}={}", escape(value))
}

/// En-têtes et clef : directions non imprimées au début de la première mesure.
///
/// `<other-direction>` est le point d'extension normalisé pour une direction
/// que MusicXML ne décrit pas (XSD 4.0, `complexType other-direction`).
pub(crate) fn write_header_directions(out: &mut Out, gregorian: &Gregorian) {
    out.attr(false);
    for header in &gregorian.headers {
        let text = format!(
            "{PREFIX}header:{}|{}",
            field('n', &header.name),
            field('v', &header.value)
        );
        write_direction(out, &text);
    }
    // Une source sans jeton de clé (`( )`) ne doit pas en recevoir une par
    // défaut : la clé absente est elle-même une donnée du payload.
    if let Some(clef) = gregorian.clefs.first() {
        let text = format!(
            "{PREFIX}clef:{}|{}",
            field('t', clef),
            field('l', &gregorian.staff_lines.to_string())
        );
        write_direction(out, &text);
    }
}

/// Une syllabe et ses barres : une direction par mesure dérivée.
///
/// La projection dérivée crée une mesure par syllabe (y compris les groupes
/// qui ne contiennent qu'une barre) : l'index de mesure est donc l'index de
/// la syllabe, sans ambiguïté.
pub(crate) fn write_syllable_direction(out: &mut Out, gregorian: &Gregorian, index: usize) {
    let Some(syllable) = gregorian.syllables.get(index) else {
        return;
    };
    let mut text = format!(
        "{PREFIX}syllable:{}|{}|{}",
        field('i', &index.to_string()),
        field('t', &syllable.text),
        field('w', if syllable.space_after { "1" } else { "0" })
    );
    if let Some(clef) = &syllable.clef {
        text.push('|');
        text.push_str(&field('c', clef));
    }
    if let Some(rubric) = &syllable.rubric {
        text.push('|');
        text.push_str(&field('r', rubric));
    }
    for bar in &syllable.bars {
        text.push('|');
        text.push_str(&field('b', bar.as_str()));
    }
    for sign in &syllable.signs {
        text.push('|');
        text.push_str(&field('z', &sign.raw));
    }
    out.attr(false);
    write_direction(out, &text);
    // Une alteration (`Gabc.tex:481-489,512-540`) est un glyphe, pas une note
    // chantée : la projection dérivée ne produit donc aucun `<note>` pour
    // elle. Son rang source est transporté séparément pour que l'aller-retour
    // L6 reste exact ; aucun consommateur tiers n'a besoin de ce champ.
    for (note_index, note) in syllable.notes.iter().enumerate() {
        if note
            .signs
            .iter()
            .any(|sign| sign.kind == SignKind::Alteration)
        {
            let text = format!(
                "{PREFIX}alter:{}|{}|{}|{}",
                field('i', &index.to_string()),
                field('a', &note_index.to_string()),
                field('l', &note.letter.to_string()),
                field('g', &note.glyph)
            );
            out.attr(false);
            write_direction(out, &text);
        }
    }
}

/// Lettre et glyphe d'une note : `<other-notation>` non imprimé.
///
/// Les signes rythmiques vont dans `<technical><other-technical>` ci-dessous ;
/// les formes que MusicXML énumère vont dans `<notehead>` (XSD 4.0,
/// `notehead-value`).
pub(crate) fn write_note_notation(
    out: &mut Out,
    note: &GregorianNote,
    syllable_index: usize,
    note_index: usize,
) {
    let text = format!(
        "{PREFIX}note:{}|{}|{}|{}",
        field('l', &note.letter.to_string()),
        field('g', &note.glyph),
        field('i', &syllable_index.to_string()),
        field('a', &note_index.to_string())
    );
    out.xml
        .start_element("notations", &[("print-object", "no")]);
    out.xml
        .tag_with("other-notation", &[("type", "single")], &text);
    out.xml.end_element();
}

/// Signes attachés à une note : `<other-technical>` (XSD 4.0,
/// `complexType other-technical`).
pub(crate) fn write_note_signs(out: &mut Out, note: &GregorianNote) {
    if note.signs.is_empty() {
        return;
    }
    out.xml
        .start_element("notations", &[("print-object", "no")]);
    out.xml.start_element("technical", &[]);
    for sign in &note.signs {
        out.xml.tag_with(
            "other-technical",
            &[("print-object", "no")],
            &format!("{PREFIX}sign:{}", field('s', &sign.raw)),
        );
    }
    out.xml.end_element();
    out.xml.end_element();
}

fn write_direction(out: &mut Out, text: &str) {
    out.xml
        .start_element("direction", &[("print-object", "no")]);
    out.xml.start_element("direction-type", &[]);
    out.xml
        .tag_with("other-direction", &[("print-object", "no")], text);
    out.xml.end_element();
    out.xml.end_element();
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, NeumeSign, SignKind, Syllable};

    #[test]
    fn escape_keeps_field_boundaries_unambiguous() {
        assert_eq!(escape("a|b\\c\nd>e"), "a\\|b\\\\c\\nd\\ge");
        assert_eq!(field('t', "<sp>R/</sp>"), "t=\\lsp\\gR/\\l/sp\\g");
    }

    #[test]
    fn no_raw_gabc_source_in_xml() {
        use scoretk_core::model::{
            Extension, GregorianHeader, GregorianNote, LyricsSyllabic, Syllable,
        };

        let gregorian = Gregorian {
            headers: vec![
                GregorianHeader {
                    name: "name".to_owned(),
                    value: "Veni Sancte Spiritus".to_owned(),
                },
                GregorianHeader {
                    name: "language".to_owned(),
                    value: "latin".to_owned(),
                },
            ],
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "<sc>Ve</sc>ni".to_owned(),
                notes: vec![GregorianNote {
                    letter: 'g',
                    glyph: "h_".to_owned(),
                    signs: vec![NeumeSign::new(SignKind::Ictus, "'")],
                }],
                space_after: true,
                bars: vec![BarKind::DivisioFinalis],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let mut score = gregorian.build_derived_score();
        score.extensions.format = Some(Extension::Gregorian(gregorian.clone()));
        let xml = crate::export_musicxml(&score).expect("MusicXML export");
        // Not a single raw GABC delimiter or source group is copied: each
        // fact is rebuilt into its own MusicXML element/field.
        assert!(!xml.contains("%%"), "{xml}");
        assert!(!xml.contains("(g"), "{xml}");
        assert!(!xml.contains("<sc>Ve</sc>ni(h_')"), "{xml}");
        assert!(xml.contains("gabc:syllable:i=0"), "{xml}");
        assert!(xml.contains("gabc:note:l=g|g=h_"), "{xml}");
        assert!(xml.contains("gabc:sign:s='"), "{xml}");
        assert!(xml.contains("<senza-misura"), "{xml}");
        assert_eq!(
            score.measures[0]
                .segments
                .iter()
                .filter_map(|segment| {
                    match segment.element(scoretk_core::model::Track::new(
                        scoretk_core::model::StaffId::from_raw(0),
                        0,
                    )) {
                        Some(scoretk_core::model::SegmentItem::ChordRest(chord_rest)) => {
                            Some(chord_rest)
                        }
                        _ => None,
                    }
                })
                .filter(|chord_rest| {
                    chord_rest
                        .lyrics
                        .first()
                        .is_some_and(|lyric| lyric.syllabic == LyricsSyllabic::Single)
                })
                .count(),
            1
        );
    }

    /// Out-of-reach L6 facts are declared, not silently counted as gains:
    /// custos and line breaks (`Gabc.tex:872-906`), bracketed TeX/layout
    /// attributes (`:689-700,908-931`), NABC cuts
    /// (`gabc-score-determination.y:870-907`) and the exact order of bars
    /// inside a group. None of them is a sung fact and none gets an XML field.
    #[test]
    fn out_of_reach_gabc_facts_have_no_xml_field() {
        let mut out = Out::new();
        let gregorian = Gregorian {
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "<sp>V/</sp>.".to_owned(),
                bars: vec![BarKind::DivisioFinalis],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        write_header_directions(&mut out, &gregorian);
        write_syllable_direction(&mut out, &gregorian, 0);
        let xml = out.xml.finish();
        for absent in [
            "gabc:custos",
            "gabc:line-break",
            "gabc:nabc",
            "gabc:attribute",
        ] {
            assert!(
                !xml.contains(absent),
                "{absent} must not be invented: {xml}"
            );
        }
    }

    #[test]
    fn a_syllable_direction_carries_each_fact_separately() {
        let gregorian = Gregorian {
            syllables: vec![Syllable {
                text: "De|us".to_owned(),
                clef: Some("c3".to_owned()),
                rubric: Some("<sp>R/</sp>".to_owned()),
                bars: vec![BarKind::DivisioFinalis, BarKind::Dominican(2)],
                signs: vec![NeumeSign::new(SignKind::Ictus, "'")],
                space_after: true,
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let mut out = Out::new();
        write_syllable_direction(&mut out, &gregorian, 0);
        let xml = out.xml.finish();
        assert!(xml.contains("gabc:syllable:i=0|t=De\\|us|w=1"), "{xml}");
        assert!(xml.contains("|c=c3"), "{xml}");
        assert!(xml.contains("|r=\\lsp\\gR/\\l/sp\\g"), "{xml}");
        assert!(xml.contains("|b=::"), "{xml}");
        assert!(xml.contains("|b=;2"), "{xml}");
        assert!(xml.contains("|z='"), "{xml}");
    }
}
