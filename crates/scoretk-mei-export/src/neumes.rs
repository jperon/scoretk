// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : écriture du module NEUMES de MEI 5.1 depuis le
// flux L6 grégorien, d'après les *Guidelines* 5.1 ch. 6 (§6.1, §6.1.1,
// §6.4.1–§6.4.10) et `mei-all-5.1.rng`, rédigé en 2026.
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

//! Écriture neumatique : `<syllable>` / `<syl>` / `<neume>` / `<nc>` dans une
//! `<section>` MEI 5.1 (`Guidelines` §6.4.10), sans aucune altération du
//! chemin CMN utilisé pour les partitions sans payload L6.
//!
//! MEI 5.1 porte les hauteurs (`@pname`/`@oct`, §6.4.2), les modificateurs
//! `<liquescent>`, `<quilisma>`, `<oriscus>`, `<strophicus>` (§6.4.1), les
//! épisèmes `<episema form="h|v">` (§6.4.4), les clés `<clef shape="C|F"
//! line>` (§4.2.2, §6.4.10) et les divisions `<divLine form>` (§6.4.1). Les
//! jetons GABC sans équivalent normatif (orthographe exacte d'un glyphe,
//! chiffres de placement, clé double ou basse, barres dominicaines, en-têtes)
//! sont émis dans des `<annot type="gabc-*">`, extensions scoretk déclarées
//! dans `scoretk-mei-common/src/neumes.rs`.

use scoretk_core::model::{Gregorian, GregorianNote, NeumeSign, SignKind, Syllable};
use scoretk_mei_common::{ids, neumes};
use scoretk_xml_common::XmlWriter;

use crate::MeiOptions;

/// Écrit un document MEI 5.1 complet pour un chant grégorien.
pub(super) fn write(
    gregorian: &Gregorian,
    options: &MeiOptions,
) -> Result<String, scoretk_mei_common::Error> {
    let mut xml = XmlWriter::new();
    xml.processing_instruction(
        "xml-model href=\"https://music-encoding.org/schema/5.1/mei-all.rng\" \
         type=\"application/xml\" schematypens=\"http://purl.oclc.org/dsdl/schematron\"",
    );
    xml.start_element(
        "mei",
        &[
            ("xmlns", ids::MEI_NAMESPACE),
            ("meiversion", neumes::NEUMES_VERSION),
        ],
    );
    write_head(&mut xml, gregorian, &options.encoding_date);

    xml.start_element("music", &[]);
    xml.start_element("body", &[]);
    xml.start_element("mdiv", &[]);
    xml.start_element("score", &[]);
    // Une clé portée par une syllabe est un changement, pas une clé
    // initiale : ne pas la dupliquer dans `<scoreDef>`.
    let changes = gregorian
        .syllables
        .iter()
        .filter(|syllable| syllable.clef.is_some())
        .count();
    let initial = gregorian.clefs.len().saturating_sub(changes);
    write_score_def(&mut xml, gregorian, initial);
    xml.start_element("section", &[("xml:id", "s1")]);
    xml.start_element("staff", &[("n", "1")]);
    xml.start_element("layer", &[("n", "1")]);

    // Les clés initiales précèdent la première syllabe (`Gabc.tex:847-870`).
    for (index, token) in gregorian.clefs.iter().take(initial).enumerate() {
        annot(
            &mut xml,
            neumes::CLEF_ANNOT,
            &index.to_string(),
            token,
            None,
        );
    }

    for (syllable_index, syllable) in gregorian.syllables.iter().enumerate() {
        write_syllable(&mut xml, syllable, syllable_index, gregorian, initial);
    }
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    xml.end_element();
    Ok(xml.finish())
}

fn write_head(xml: &mut XmlWriter, gregorian: &Gregorian, date: &str) {
    xml.start_element("meiHead", &[]);
    xml.start_element("fileDesc", &[]);
    xml.start_element("titleStmt", &[]);
    let title = gregorian
        .headers
        .iter()
        .find(|header| header.name == "name")
        .map_or("Cantus", |header| header.value.as_str());
    xml.tag("title", title);
    xml.end_element();
    xml.start_element("pubStmt", &[]);
    xml.tag_empty("date", &[("isodate", date)]);
    xml.end_element();
    if !gregorian.headers.is_empty() {
        xml.start_element("notesStmt", &[]);
        for header in &gregorian.headers {
            annot(xml, neumes::HEADER_ANNOT, &header.name, &header.value, None);
        }
        xml.end_element();
    }
    xml.end_element();
    xml.end_element();
}

fn write_score_def(xml: &mut XmlWriter, gregorian: &Gregorian, initial_clefs: usize) {
    xml.start_element("scoreDef", &[]);
    xml.start_element("staffGrp", &[]);
    xml.start_element(
        "staffDef",
        &[
            ("n", "1"),
            ("lines", &gregorian.staff_lines.clamp(2, 5).to_string()),
        ],
    );
    if let Some(token) = gregorian.clefs.iter().take(initial_clefs).next() {
        if let Some(parts) = neumes::clef_parts(token) {
            xml.tag_empty(
                "clef",
                &[
                    ("shape", &parts.shape.to_string()),
                    ("line", &parts.line.to_string()),
                ],
            );
        }
    }
    xml.end_element();
    xml.end_element();
    xml.end_element();
}

/// `<annot type="…" n="…">texte</annot>` ; `kind` ajoute un second jeton
/// `@type` pour les familles de signes.
fn annot(xml: &mut XmlWriter, kind: &str, n: &str, text: &str, sign: Option<SignKind>) {
    let type_value = sign.map_or_else(
        || kind.to_owned(),
        |family| format!("{kind} {}", neumes::sign_kind_name(family)),
    );
    xml.tag_with("annot", &[("type", &type_value), ("n", n)], text);
}

fn write_syllable(
    xml: &mut XmlWriter,
    syllable: &Syllable,
    syllable_index: usize,
    gregorian: &Gregorian,
    initial_clefs: usize,
) {
    xml.start_element("syllable", &[]);
    // Les annotations sont des enfants légaux de <syllable> (§6.1) et
    // n'interfèrent pas avec le texte chanté lu dans <syl>.
    if let Some(above) = &syllable.above_text {
        annot(
            xml,
            neumes::ABOVE_ANNOT,
            &syllable_index.to_string(),
            above,
            None,
        );
    }
    if let Some(rubric) = &syllable.rubric {
        annot(
            xml,
            neumes::RUBRIC_ANNOT,
            &syllable_index.to_string(),
            rubric,
            None,
        );
    }
    if syllable.space_after {
        annot(
            xml,
            neumes::SPACE_ANNOT,
            &syllable_index.to_string(),
            "1",
            None,
        );
    }
    xml.tag_with("syl", &[], &syllable.text);

    let current_clef = current_clef(syllable, gregorian, initial_clefs);
    if let Some(token) = &syllable.clef {
        if let Some(parts) = neumes::clef_parts(token) {
            xml.tag_empty(
                "clef",
                &[
                    ("shape", &parts.shape.to_string()),
                    ("line", &parts.line.to_string()),
                ],
            );
        }
        let change_index = initial_clefs
            + gregorian
                .syllables
                .iter()
                .take(syllable_index)
                .filter(|candidate| candidate.clef.is_some())
                .count();
        annot(
            xml,
            neumes::CLEF_ANNOT,
            &change_index.to_string(),
            token,
            None,
        );
    }

    if !syllable.notes.is_empty() {
        xml.start_element("neume", &[]);
        for note in &syllable.notes {
            write_nc(xml, note, &current_clef);
        }
        xml.end_element();
        for (note_index, note) in syllable.notes.iter().enumerate() {
            let token = format!("{}{}", note.letter, note.glyph);
            annot(
                xml,
                neumes::NOTE_ANNOT,
                &note_index.to_string(),
                &token,
                None,
            );
            if let (Some(start), Some(end)) = (note.source_offset, note.source_end) {
                annot(
                    xml,
                    neumes::SOURCE_ANNOT,
                    &note_index.to_string(),
                    &format!("{start}:{end}"),
                    None,
                );
            }
            for (sign_index, sign) in note.signs.iter().enumerate() {
                write_sign_annot(
                    xml,
                    neumes::NOTE_SIGN_ANNOT,
                    &format!("{note_index}:{sign_index}"),
                    sign,
                );
            }
        }
    }

    for (bar_index, bar) in syllable.bars.iter().enumerate() {
        xml.tag_empty("divLine", &[("form", neumes::bar_form(*bar))]);
        annot(
            xml,
            neumes::BAR_ANNOT,
            &bar_index.to_string(),
            bar.as_str(),
            None,
        );
    }
    for (sign_index, sign) in syllable.signs.iter().enumerate() {
        if let Some(form) = neumes::episema_form(sign.kind) {
            xml.tag_empty("episema", &[("form", form)]);
        }
        write_sign_annot(xml, neumes::GROUP_SIGN_ANNOT, &sign_index.to_string(), sign);
    }
    xml.end_element();
}

fn write_nc(xml: &mut XmlWriter, note: &GregorianNote, clef: &str) {
    let (pname, oct) = neumes::nc_position(note.letter, clef);
    let mut attributes = vec![
        ("pname".to_owned(), pname.to_string()),
        ("oct".to_owned(), oct.to_string()),
    ];
    if let Some(tilt) = neumes::glyph_tilt(&note.glyph) {
        attributes.push(("tilt".to_owned(), tilt.to_owned()));
    }
    if let Some(curve) = neumes::glyph_curve(&note.glyph) {
        attributes.push(("curve".to_owned(), curve.to_owned()));
    }
    let refs: Vec<(&str, &str)> = attributes
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("nc", &refs);
    write_modifiers(xml, &note.signs);
    xml.end_element();
}

/// Modificateurs standards d'un `<nc>` (`Guidelines` §6.4.1) et de la
/// syllabe (§6.4.4). Un signe sans élément MEI dédié reste dans l'annotation.
fn write_modifiers(xml: &mut XmlWriter, signs: &[NeumeSign]) {
    for sign in signs {
        if let Some(element) = neumes::component_modifier(sign.kind) {
            xml.tag_empty(element, &[]);
        } else if let Some(form) = neumes::episema_form(sign.kind) {
            xml.tag_empty("episema", &[("form", form)]);
        }
    }
}

fn write_sign_annot(xml: &mut XmlWriter, kind: &str, n: &str, sign: &NeumeSign) {
    annot(xml, kind, n, &sign.raw, Some(sign.kind));
}

/// Clé en vigueur pour une syllabe, sous forme de jeton GABC.
fn current_clef(syllable: &Syllable, gregorian: &Gregorian, initial: usize) -> String {
    if let Some(token) = &syllable.clef {
        return token.clone();
    }
    let changes_before = gregorian
        .syllables
        .iter()
        .take_while(|candidate| !std::ptr::eq(*candidate, syllable))
        .filter(|candidate| candidate.clef.is_some())
        .count();
    let index = initial + changes_before;
    gregorian.clefs.get(index).cloned().unwrap_or_else(|| {
        gregorian
            .clefs
            .first()
            .cloned()
            .unwrap_or_else(|| "c4".to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, GregorianHeader, NeumeSign};

    fn chant() -> Gregorian {
        Gregorian {
            headers: vec![GregorianHeader {
                name: "name".to_owned(),
                value: "Veni Creator".to_owned(),
            }],
            staff_lines: 4,
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "Ve<sp>R/</sp>ni".to_owned(),
                notes: vec![GregorianNote {
                    letter: 'g',
                    glyph: "w.".to_owned(),
                    signs: vec![NeumeSign::new(SignKind::Quilisma, "w")],
                    ..GregorianNote::default()
                }],
                bars: vec![BarKind::DivisioMinima],
                rubric: Some("<sp>R/</sp>".to_owned()),
                space_after: true,
                ..Syllable::default()
            }],
            derived_modern_view: false,
        }
    }

    #[test]
    fn writes_the_mei_five_neume_module_skeleton() {
        let output = write(&chant(), &MeiOptions::default()).expect("MEI neumes");
        assert!(output.trim_end().ends_with("</mei>"), "the root is closed");
        assert!(output.contains("meiversion=\"5.1\""));
        assert!(output.contains("mei-all.rng"));
        assert!(output.contains("<clef shape=\"C\" line=\"3\"/>"));
        assert!(output.contains("<syl>Ve&lt;sp&gt;R/&lt;/sp&gt;ni</syl>"));
        assert!(output.contains("<nc pname=\"b\" oct=\"3\">"));
        assert!(output.contains("<quilisma/>"));
        assert!(output.contains("<divLine form=\"minima\"/>"));
        assert!(output.contains("type=\"gabc-header\" n=\"name\">Veni Creator"));
        assert!(output.contains("type=\"gabc-note\" n=\"0\">gw.</annot>"));
    }
}
