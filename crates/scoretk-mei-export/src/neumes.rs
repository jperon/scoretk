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
//! Les porteurs sont ceux du module NEUMES : `<neume type>` (une par groupe
//! de glyphes GABC), `<nc @pname @oct @tilt @curve @con>`, les enfants
//! `<liquescent>`, `<quilisma>`, `<oriscus>`, `<strophicus>` et `<signifLet>`
//! (§6.4.1, §6.4.8), `<episema @form @place>` (§6.4.4), `<divLine @form>`
//! (§6.4.1), `<clef shape line>` (§4.2.2, §6.4.10), `<syl @wordpos @con>`
//! et les éléments standards de `<meiHead>` (ch. 3). Les en-têtes et les
//! glyphes GABC sont ainsi reconstruits depuis des propriétés MEI, jamais
//! copiés dans une annotation privée (option `gabc_tokens`, désactivée).

use scoretk_core::model::{Gregorian, GregorianNote, NeumeSign, SignKind, Syllable};
use scoretk_mei_common::{ids, neumes};

use super::nabc;
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
    let private = options.gabc_private;
    write_head(
        &mut xml,
        gregorian,
        &options.encoding_date,
        options.gabc_tokens && private,
        private,
    );

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
    // A manuscript NABC voice is written only in the additive mode, in a
    // second, parallel, unpitched staff (Guidelines ch. 6). Without NABC, or
    // with the default options, the output is byte-identical to master.
    let two_staves = options.full_support && nabc::has_nabc(gregorian);
    if two_staves {
        nabc::write_score_def(&mut xml, gregorian, initial);
    } else {
        write_score_def(&mut xml, gregorian, initial);
    }
    xml.start_element("section", &[("xml:id", "s1")]);
    xml.start_element("staff", &[("n", "1")]);
    // L'espacement exact entre le groupe de clé initial et la première
    // syllabe n'a pas d'élément MEI dédié : il est porté par `@label`, attribut
    // standard (`att.labelled`) et non par une annotation privée.
    let mut layer_attributes = vec![("n".to_owned(), "1".to_owned())];
    if private {
        if let Some(separator) = &gregorian.opening_separator {
            layer_attributes.push(("label".to_owned(), neumes::layout_label("open", separator)));
        }
    }
    let layer_refs: Vec<(&str, &str)> = layer_attributes
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("layer", &layer_refs);

    if options.gabc_tokens && private {
        for (index, token) in gregorian.clefs.iter().take(initial).enumerate() {
            annot(
                &mut xml,
                neumes::CLEF_ANNOT,
                &index.to_string(),
                token,
                None,
            );
        }
    }

    for (syllable_index, syllable) in gregorian.syllables.iter().enumerate() {
        write_syllable(
            &mut xml,
            &SyllableContext {
                syllable,
                syllable_index,
                gregorian,
                initial_clefs: initial,
                private,
                tokens: options.gabc_tokens && private,
                linked: two_staves,
            },
        );
    }
    // staff 1 : layer, staff
    xml.end_element();
    xml.end_element();
    if two_staves {
        nabc::write_staff(&mut xml, gregorian, private);
    }
    // section, score, mdiv, body, music, mei
    for _ in 0..6 {
        xml.end_element();
    }
    Ok(xml.finish())
}

/// En-tête MEI 5.1 standard : `fileDesc/titleStmt/title`, `respStmt/persName`,
/// `workList/work/title`, `work/classification/termList/term` et
/// `sourceDesc/source/bibl/p` (*Guidelines* ch. 3, structure validée par
/// `mei-all-5.1.rng`). `@n` garde l'ordre des champs GABC pour la lecture
/// inverse ; aucun `<annot type="gabc-header">` n'est nécessaire.
fn write_head(xml: &mut XmlWriter, gregorian: &Gregorian, date: &str, tokens: bool, private: bool) {
    // `@label` est un attribut MEI standard (`att.labelled`, `data.string`) :
    // il transporte les commentaires, lignes vides, terminateurs `;;` et le
    // préfixe de notation. En `full_support` il est retiré : l'en-tête n'a pas
    // de porteur MEI pour ces détails et la perte est mesurée comme telle.
    let mut head_attributes = Vec::new();
    if private {
        if let Some(label) = scoretk_mei_common::gabc_layout::header_label(gregorian) {
            head_attributes.push(("label".to_owned(), label));
        }
    }
    let head_refs: Vec<(&str, &str)> = head_attributes
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("meiHead", &head_refs);
    xml.start_element("fileDesc", &[]);
    xml.start_element("titleStmt", &[]);
    for (index, header) in gregorian.headers.iter().enumerate() {
        if header.name == "name" {
            xml.tag_with("title", &[("n", &index.to_string())], &header.value);
            break;
        }
    }
    let agent = |name: &str| matches!(name, "author" | "transcriber");
    if gregorian.headers.iter().any(|header| agent(&header.name)) {
        xml.start_element("respStmt", &[]);
        for (index, header) in gregorian.headers.iter().enumerate() {
            if agent(&header.name) {
                xml.tag_with(
                    "persName",
                    &[("role", &header.name), ("n", &index.to_string())],
                    &header.value,
                );
            }
        }
        xml.end_element();
    }
    xml.end_element();
    xml.start_element("pubStmt", &[]);
    xml.tag_empty("date", &[("isodate", date)]);
    xml.end_element();

    let notes: Vec<_> = gregorian
        .headers
        .iter()
        .enumerate()
        .filter(|(_, header)| {
            !matches!(
                header.name.as_str(),
                "name"
                    | "office-part"
                    | "book"
                    | "mode"
                    | "annotation"
                    | "occasion"
                    | "style"
                    | "author"
                    | "transcriber"
            )
        })
        .collect();
    if !notes.is_empty() {
        xml.start_element("sourceDesc", &[]);
        xml.start_element("source", &[]);
        xml.start_element("bibl", &[]);
        for (index, header) in &notes {
            xml.tag_with(
                "rend",
                &[("type", &header.name), ("n", &index.to_string())],
                &header.value,
            );
        }
        xml.end_element();
        xml.end_element();
        xml.end_element();
    }
    if tokens {
        xml.start_element("notesStmt", &[]);
        for header in &gregorian.headers {
            annot(xml, neumes::HEADER_ANNOT, &header.name, &header.value, None);
        }
        xml.end_element();
    }
    xml.end_element();

    // `workList` est un enfant direct de `meiHead`, après `fileDesc`.
    let work = |name: &str| matches!(name, "office-part" | "book");
    let classified = |name: &str| matches!(name, "mode" | "annotation" | "occasion" | "style");
    if gregorian
        .headers
        .iter()
        .any(|header| work(&header.name) || classified(&header.name))
    {
        xml.start_element("workList", &[]);
        xml.start_element("work", &[]);
        let mut first = true;
        for (index, header) in gregorian.headers.iter().enumerate() {
            if work(&header.name) || (classified(&header.name) && first) {
                xml.tag_with(
                    "title",
                    &[("type", &header.name), ("n", &index.to_string())],
                    &header.value,
                );
                first = false;
            }
        }
        let terms: Vec<_> = gregorian
            .headers
            .iter()
            .enumerate()
            .filter(|(_, header)| classified(&header.name))
            .collect();
        if !terms.is_empty() {
            xml.start_element("classification", &[]);
            xml.start_element("termList", &[]);
            for (index, header) in terms {
                xml.tag_with(
                    "term",
                    &[("type", &header.name), ("n", &index.to_string())],
                    &header.value,
                );
            }
            xml.end_element();
            xml.end_element();
        }
        xml.end_element();
        xml.end_element();
    }
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
            let shape = parts.shape.to_string();
            let line = parts.line.to_string();
            let mut attributes = vec![("shape", shape.as_str()), ("line", line.as_str())];
            if neumes::clef_is_flat(token) {
                attributes.push(("type", "clef-flat"));
            }
            xml.tag_empty("clef", &attributes);
        }
    }
    xml.end_element();
    xml.end_element();
    xml.end_element();
}

/// `<annot type="…" n="…">texte</annot>` : extension scoretk, émise
/// uniquement quand `MeiOptions::gabc_tokens` est vrai.
fn annot(xml: &mut XmlWriter, kind: &str, n: &str, text: &str, sign: Option<SignKind>) {
    let type_value = sign.map_or_else(
        || kind.to_owned(),
        |family| format!("{kind} {}", neumes::sign_kind_name(family)),
    );
    xml.tag_with("annot", &[("type", &type_value), ("n", n)], text);
}

/// Contexte d'une syllabe pour l'écriture NEUMES.
struct SyllableContext<'a> {
    syllable: &'a Syllable,
    syllable_index: usize,
    gregorian: &'a Gregorian,
    initial_clefs: usize,
    /// Porteurs source-privés (`@label`, `@type="gabc-*"`, jetons NABC).
    private: bool,
    /// Annotations strictement privées (`<annot type="gabc-*">`).
    tokens: bool,
    linked: bool,
}

fn write_syllable(xml: &mut XmlWriter, context: &SyllableContext<'_>) {
    let SyllableContext {
        syllable,
        syllable_index,
        gregorian,
        initial_clefs,
        private,
        tokens,
        linked,
    } = *context;
    let elision = syllable.text.contains('~');
    // Le séparateur exact entre ce groupe et le suivant (`Gabc.tex:60-61`)
    // n'a pas de champ MEI ; `@label` est un attribut standard et garde son
    // spelling sans ajouter d'annotation `gabc-*` au document. Il est porté
    // par `<syl>` et non par `<syllable>` pour laisser intacte la structure
    // neumatique attendue par les lecteurs du module NEUMES.
    let mut separator_label = if private {
        syllable
            .separator_after
            .as_ref()
            .map(|separator| neumes::layout_label("sep", separator))
    } else {
        None
    };
    // Le détail GABC (attributs `[…]`, jetons de groupe, espace avant `(`)
    // partage le même `@label` standard, après le séparateur de §19 : le
    // `<syllable>` reste exactement `<syllable>` pour les lecteurs tiers.
    let detail = if private {
        scoretk_mei_common::gabc_layout::attributes_label(syllable)
    } else {
        None
    };
    if let Some(detail) = &detail {
        match &mut separator_label {
            Some(label) => {
                label.push('|');
                label.push_str(detail);
            }
            None => separator_label = Some(detail.clone()),
        }
    }
    let mut syl_attributes = vec![
        ("wordpos", if syllable.space_after { "t" } else { "m" }),
        (
            "con",
            if elision {
                "t"
            } else if syllable.space_after {
                "s"
            } else {
                "d"
            },
        ),
    ];
    if let Some(label) = &separator_label {
        syl_attributes.push(("label", label.as_str()));
    }
    let attributes = if linked {
        nabc::link(syllable_index, 1)
    } else {
        Vec::new()
    };
    let link_refs: Vec<(&str, &str)> = attributes
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("syllable", &link_refs);
    if private {
        if let Some(above) = &syllable.above_text {
            xml.tag_with(
                "signifLet",
                &[("type", "gabc-above"), ("place", "above")],
                above,
            );
        }
    }
    // `tag_with` garde le texte sur une seule ligne : l'indentation de
    // `end_element` deviendrait un texte de syllable et décalerait les paroles.
    // Les styles réversibles passent par `<rend>` (`Guidelines` §6.4.10).
    // En mode full-support les `@type="gabc-i"` sont des vidages : le texte
    // chanté reste dans `<syl>` sans balisage source, la perte de style est
    // comptée comme un défaut.
    let pieces = neumes::syl_pieces(&syllable.text);
    let plain_text: String = pieces
        .iter()
        .map(|piece| match piece {
            neumes::SylPiece::Plain(text) => text.as_str(),
            neumes::SylPiece::Styled { text, .. } => text.as_str(),
        })
        .collect();
    let plain_only = pieces.is_empty()
        || matches!(pieces.as_slice(), [neumes::SylPiece::Plain(plain)] if plain == &syllable.text);
    if !private {
        xml.tag_with("syl", &syl_attributes, &plain_text);
    } else if plain_only {
        xml.tag_with("syl", &syl_attributes, &syllable.text);
    } else {
        xml.start_element("syl", &syl_attributes);
        for piece in &pieces {
            match piece {
                neumes::SylPiece::Plain(text) => {
                    xml.tag_with("rend", &[("type", "plain")], text);
                }
                neumes::SylPiece::Styled {
                    attributes, text, ..
                } => {
                    let refs: Vec<(&str, &str)> = attributes
                        .iter()
                        .map(|(key, value)| (*key, value.as_str()))
                        .collect();
                    xml.tag_with("rend", &refs, text);
                }
            }
        }
        xml.end_element();
    }

    let current_clef = current_clef(syllable, gregorian, initial_clefs);
    if let Some(token) = &syllable.clef {
        if let Some(parts) = neumes::clef_parts(token) {
            let shape = parts.shape.to_string();
            let line = parts.line.to_string();
            let mut attributes = vec![("shape", shape.as_str()), ("line", line.as_str())];
            if neumes::clef_is_flat(token) {
                attributes.push(("type", "clef-flat"));
            }
            xml.tag_empty("clef", &attributes);
        }
        if tokens {
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
    }

    if !syllable.notes.is_empty() {
        // Un `<neume>` par groupe de glyphes GABC : `/`, `//`, `/0` et `!`
        // séparent deux groupes (`Gabc.tex:668-687`), pas deux syllabes.
        let mut starts = vec![true; syllable.notes.len()];
        for index in 1..syllable.notes.len() {
            starts[index] = has_space(&syllable.notes[index - 1].glyph);
        }
        for (index, note) in syllable.notes.iter().enumerate() {
            if starts[index] {
                if index > 0 {
                    xml.end_element();
                }
                let end = syllable.notes[index + 1..]
                    .iter()
                    .position(|candidate| has_space(&candidate.glyph))
                    .map_or(syllable.notes.len(), |offset| index + 1 + offset);
                let group = &syllable.notes[index..end];
                let glyphs: Vec<String> = group
                    .iter()
                    .map(|candidate| format!("{}{}", candidate.letter, candidate.glyph))
                    .collect();
                let number = syllable.notes[..index]
                    .iter()
                    .filter(|candidate| has_space(&candidate.glyph))
                    .count();
                xml.start_element(
                    "neume",
                    &[
                        ("n", &number.to_string()),
                        ("type", neumes::neume_type(group.len(), &glyphs)),
                    ],
                );
            }
            write_nc(
                xml,
                note,
                &current_clef,
                index > 0 && !starts[index],
                private,
            );
        }
        xml.end_element();
        if tokens {
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
    }

    for (bar_index, bar) in syllable.bars.iter().enumerate() {
        xml.tag_empty("divLine", &[("form", neumes::bar_form(*bar))]);
        if tokens {
            annot(
                xml,
                neumes::BAR_ANNOT,
                &bar_index.to_string(),
                bar.as_str(),
                None,
            );
        }
    }
    for (sign_index, sign) in syllable.signs.iter().enumerate() {
        if let Some(form) = neumes::episema_form(sign.kind) {
            let place = if sign.raw.contains('0') {
                "below"
            } else {
                "above"
            };
            xml.tag_empty("episema", &[("form", form), ("place", place)]);
        }
        if tokens {
            write_sign_annot(xml, neumes::GROUP_SIGN_ANNOT, &sign_index.to_string(), sign);
        }
    }
    if tokens {
        if let Some(rubric) = &syllable.rubric {
            annot(
                xml,
                neumes::RUBRIC_ANNOT,
                &syllable_index.to_string(),
                rubric,
                None,
            );
        }
    }
    xml.end_element();
}

/// Vrai si un glyphe GABC contient une coupure de groupe (`/`, `//`, `/!`).
fn has_space(glyph: &str) -> bool {
    neumes::glyph_parts(glyph).iter().any(|part| {
        part.attributes
            .iter()
            .any(|(key, value)| *key == "type" && value == "gabc-space")
    })
}

fn write_nc(xml: &mut XmlWriter, note: &GregorianNote, clef: &str, connected: bool, private: bool) {
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
    if connected {
        attributes.push(("con".to_owned(), "g".to_owned()));
    }
    // Une lettre majuscule GABC est un punctum inclinatum (`Gabc.tex:454-468`).
    // MEI 5.1 le décrit par la forme du trait (`@tilt="se"`, §6.4.2) et par
    // le type de neume ; le type porte la casse pour la lecture inverse.
    if note.letter.is_ascii_uppercase() {
        attributes.push(("type".to_owned(), "punctum-inclinatum".to_owned()));
        if !attributes.iter().any(|(key, _)| key == "tilt") {
            attributes.push(("tilt".to_owned(), "se".to_owned()));
        }
    }
    let refs: Vec<(&str, &str)> = attributes
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    xml.start_element("nc", &refs);
    for part in neumes::glyph_parts(&note.glyph) {
        // `gabc-space`, `gabc-fusion`, `gabc-nabc` encode a source spelling
        // no MEI reader can interpret. Full support omits them; the exact
        // glyph remains a measured defect instead of a hidden dump.
        if !private
            && part
                .attributes
                .iter()
                .any(|(key, value)| key == &"type" && value.starts_with("gabc-"))
        {
            continue;
        }
        let attrs: Vec<(&str, String)> = part.attributes;
        let refs: Vec<(&str, &str)> = attrs
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        if part.text.is_empty() {
            xml.tag_empty(part.element, &refs);
        } else {
            xml.tag_with(part.element, &refs, &part.text);
        }
    }
    // Les signes sans glyphe source (partition assemblée depuis un autre
    // format) gardent leurs éléments MEI standards.
    if note.glyph.is_empty() {
        write_modifiers(xml, &note.signs);
    }
    xml.end_element();
}

/// Modificateurs standards d'un `<nc>` (`Guidelines` §6.4.1) pour les signes
/// assemblés sans glyphe GABC.
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
            opening_separator: None,
            derived_modern_view: false,
            ..Gregorian::default()
        }
    }

    #[test]
    fn writes_the_mei_five_neume_module_skeleton() {
        let options = MeiOptions {
            gabc_tokens: true,
            ..MeiOptions::default()
        };
        let output = write(&chant(), &options).expect("MEI neumes");
        assert!(output.trim_end().ends_with("</mei>"), "the root is closed");
        assert!(output.contains("meiversion=\"5.1\""));
        assert!(output.contains("mei-all.rng"));
        assert!(output.contains("<clef shape=\"C\" line=\"3\"/>"));
        assert!(output.contains("Veni Creator"));
        assert!(output.contains("<nc pname=\"b\" oct=\"3\">"));
        assert!(output.contains("<quilisma/>"));
        assert!(output.contains("<divLine form=\"minima\"/>"));
        assert!(output.contains("type=\"gabc-note\" n=\"0\">gw.</annot>"));
    }

    #[test]
    fn default_output_has_no_private_annotation() {
        let output = write(&chant(), &MeiOptions::default()).expect("MEI neumes");
        assert!(!output.contains("gabc-"), "the standard carriers suffice");
        assert!(output.contains("<syl wordpos=\"t\" con=\"s\">"));
        assert!(output.contains("<quilisma/>"));
    }

    #[test]
    fn exact_body_layout_is_carried_by_standard_labels() {
        let mut gregorian = chant();
        gregorian.opening_separator = Some(" ".to_owned());
        gregorian.syllables[0].separator_after = Some("\n\n".to_owned());
        let output = write(&gregorian, &MeiOptions::default()).expect("MEI neumes");
        assert!(output.contains("<layer n=\"1\" label=\"layout:open:S\">"));
        assert!(output.contains("<syl wordpos=\"t\" con=\"s\" label=\"layout:sep:LL\">"));
        assert!(!output.contains("gabc-"), "no private annotation is added");
        assert!(
            output.contains("<syllable>"),
            "the neume skeleton is unchanged"
        );
    }
}
