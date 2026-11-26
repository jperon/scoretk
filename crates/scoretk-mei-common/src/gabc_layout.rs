// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : porteurs MEI des champs de mise en page GABC
// qui n'ont pas d'équivalent dans MEI 5.1. Écrits en 2026.
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

//! Transport des champs de layout GABC par `@label` (`att.labelled/@label`,
//! `data.string`, *Guidelines* ch. 2). Comme en `neumes::layout_label`, aucun
//! élément ni annotation privée n'est ajouté au document MEI : les champs
//! structurés du payload sont sérialisés dans un attribut standard, puis
//! relus par le lecteur. Le format interne est ASCII hexadécimal, donc
//! insensible à l'échappement XML.

use scoretk_core::model::{Gregorian, GregorianHeaderItem, GroupToken, GroupTokenKind, Syllable};

/// Préfixe de la valeur `@label` portant le layout du document GABC.
pub const HEADER_LABEL: &str = "gabc:layout:";

fn hex(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.as_bytes() {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("hex digit"));
        out.push(char::from_digit((byte & 15) as u32, 16).expect("hex digit"));
    }
    out
}

fn unhex(text: &str) -> Option<String> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    let digits: Vec<u8> = text.bytes().collect();
    for pair in digits.as_chunks::<2>().0 {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes.push((high * 16 + low) as u8);
    }
    String::from_utf8(bytes).ok()
}

/// Sérialise `header_items`, les terminateurs `;;` et le préfixe de notation.
///
/// Retourne `None` quand tous les champs ont leur valeur par défaut : aucun
/// attribut n'est alors écrit sur `<meiHead>`, ce qui garde les documents MEI
/// construits sans source GABC inchangés.
#[must_use]
pub fn header_label(gregorian: &Gregorian) -> Option<String> {
    if gregorian.header_items.is_empty()
        && gregorian.header_double_semicolons.is_empty()
        && gregorian.notation_prefix.is_none()
    {
        return None;
    }
    let mut inner = String::new();
    for item in &gregorian.header_items {
        match item {
            GregorianHeaderItem::Field(index) => {
                inner.push('F');
                inner.push_str(&index.to_string());
            }
            GregorianHeaderItem::Comment(text) => {
                inner.push('C');
                inner.push_str(&hex(text));
            }
            GregorianHeaderItem::Blank => inner.push('B'),
        }
        inner.push(',');
    }
    inner.push('D');
    for index in &gregorian.header_double_semicolons {
        inner.push_str(&index.to_string());
        inner.push(',');
    }
    inner.push('P');
    match &gregorian.notation_prefix {
        Some(prefix) => {
            inner.push('1');
            inner.push_str(&hex(prefix));
        }
        None => inner.push('0'),
    }
    Some(format!("{HEADER_LABEL}{}", hex(&inner)))
}

/// Restaure les champs de layout depuis un `@label` de `<meiHead>`.
///
/// Sans attribut (MEI tiers, fichier antérieur à ce porteur), les champs
/// restent à leur valeur par défaut : le writer GABC retombe sur son
/// comportement historique, comme pour `layout_from_label` en §19.
pub fn read_header_label(label: Option<&str>, gregorian: &mut Gregorian) {
    let Some(value) = label.and_then(|text| text.strip_prefix(HEADER_LABEL)) else {
        return;
    };
    let Some(inner) = unhex(value) else {
        return;
    };
    let (items, rest) = inner.split_once('D').unwrap_or((&inner, ""));
    let (doubles, prefix) = rest.split_once('P').unwrap_or((rest, "0"));
    let mut header_items = Vec::new();
    for token in items.split(',').filter(|token| !token.is_empty()) {
        let (kind, value) = token.split_at(1);
        match kind {
            "F" => {
                if let Ok(index) = value.parse() {
                    header_items.push(GregorianHeaderItem::Field(index));
                }
            }
            "C" => {
                if let Some(text) = unhex(value) {
                    header_items.push(GregorianHeaderItem::Comment(text));
                }
            }
            "B" => header_items.push(GregorianHeaderItem::Blank),
            _ => {}
        }
    }
    gregorian.header_items = header_items;
    gregorian.header_double_semicolons = doubles
        .split(',')
        .filter_map(|token| token.parse().ok())
        .collect();
    gregorian.notation_prefix = match prefix.as_bytes().first() {
        Some(b'1') => unhex(&prefix[1..]),
        _ => None,
    };
}

/// Préfixe du segment portant les attributs `[…]` et jetons de groupe
/// dans le `@label` de `<syl>` (après le porteur `layout:sep:` de §19).
pub const SYLLABLE_LABEL: &str = "gabc:detail:";

/// Extrait le segment GABC d'un `@label` composite de `<syl>`.
#[must_use]
pub fn detail_from_label(label: Option<&str>) -> Option<&str> {
    label?
        .split('|')
        .find_map(|part| part.strip_prefix(SYLLABLE_LABEL))
}

/// Sérialise les attributs `[…]` et les jetons non chantés du groupe dans un
/// `@label` de `<syllable>` (`Gabc.tex:627-700,801-906`).
#[must_use]
pub fn attributes_label(syllable: &Syllable) -> Option<String> {
    if syllable.leading_attributes.is_empty()
        && syllable.group_tokens.is_empty()
        && syllable.text_spacing.is_none()
        && syllable.notes.iter().all(|note| note.attributes.is_empty())
    {
        return None;
    }
    let mut inner = String::from("L");
    for attribute in &syllable.leading_attributes {
        inner.push_str(&hex(attribute));
        inner.push(';');
    }
    inner.push('|');
    for note in &syllable.notes {
        if note.attributes.is_empty() {
            inner.push_str("E|");
        } else {
            inner.push('A');
            for attribute in &note.attributes {
                inner.push_str(&hex(&attribute.text));
                inner.push('@');
                inner.push_str(&attribute.after.to_string());
                inner.push(';');
            }
            inner.push('|');
        }
    }
    inner.push('T');
    for token in &syllable.group_tokens {
        let kind = match token.kind {
            GroupTokenKind::Bar => 'B',
            GroupTokenKind::Clef => 'C',
            GroupTokenKind::Custos => 'U',
            GroupTokenKind::Spacing => 'S',
        };
        inner.push(kind);
        inner.push_str(&hex(&token.text));
        inner.push(':');
        inner.push_str(&token.notes_before.to_string());
        inner.push(',');
    }
    inner.push('|');
    inner.push('X');
    match &syllable.text_spacing {
        Some(spacing) => {
            inner.push('1');
            inner.push_str(&hex(spacing));
        }
        None => inner.push('0'),
    }
    inner.push('|');
    Some(format!("{SYLLABLE_LABEL}{}", hex(&inner)))
}

/// Restaure les attributs `[…]` depuis un `@label` de `<syllable>`.
/// Appelé après la lecture des `<nc>`, dont l'ordre donne l'index de note.
pub fn read_attributes_label(label: Option<&str>, syllable: &mut Syllable) {
    let Some(value) = detail_from_label(label) else {
        return;
    };
    let Some(inner) = unhex(value) else {
        return;
    };
    let mut parts = inner.split('|');
    if let Some(leading) = parts.next().and_then(|part| part.strip_prefix('L')) {
        syllable.leading_attributes = leading
            .split(';')
            .filter(|token| !token.is_empty())
            .filter_map(unhex)
            .collect();
    }
    for note in &mut syllable.notes {
        if let Some(part) = parts.next() {
            if let Some(tokens) = part.strip_prefix('A') {
                note.attributes = tokens
                    .split(';')
                    .filter(|token| !token.is_empty())
                    .filter_map(|token| {
                        let (text, after) = token.split_once('@')?;
                        Some(scoretk_core::model::NoteAttribute {
                            text: unhex(text)?,
                            after: after.parse().ok()?,
                        })
                    })
                    .collect();
            }
        }
    }
    let tokens = parts
        .next()
        .and_then(|part| part.strip_prefix('T'))
        .unwrap_or("");
    syllable.group_tokens = tokens
        .split(',')
        .filter_map(|entry| {
            let (head, count) = entry.split_once(':')?;
            let text = unhex(&head[1..])?;
            let kind = match head.as_bytes().first()? {
                b'B' => GroupTokenKind::Bar,
                b'C' => GroupTokenKind::Clef,
                b'U' => GroupTokenKind::Custos,
                _ => GroupTokenKind::Spacing,
            };
            Some(GroupToken {
                notes_before: count.parse().ok()?,
                text,
                kind,
            })
        })
        .collect();
    if let Some(spacing) = parts.next().and_then(|part| part.strip_prefix('X')) {
        syllable.text_spacing = match spacing.as_bytes().first() {
            Some(b'1') => unhex(&spacing[1..]),
            _ => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::GregorianHeader;

    #[test]
    fn header_layout_crosses_a_standard_label() {
        let gregorian = Gregorian {
            headers: vec![GregorianHeader {
                name: "name".to_owned(),
                value: "Veni".to_owned(),
            }],
            header_items: vec![
                GregorianHeaderItem::Comment("% !TEX root = main.tex".to_owned()),
                GregorianHeaderItem::Blank,
                GregorianHeaderItem::Field(0),
            ],
            header_double_semicolons: vec![0],
            notation_prefix: Some("\n\n".to_owned()),
            ..Gregorian::default()
        };
        let label = header_label(&gregorian).expect("a label");
        assert!(label.starts_with(HEADER_LABEL));
        let mut back = Gregorian::default();
        read_header_label(Some(&label), &mut back);
        assert_eq!(back.header_items, gregorian.header_items);
        assert_eq!(
            back.header_double_semicolons,
            gregorian.header_double_semicolons
        );
        assert_eq!(back.notation_prefix, gregorian.notation_prefix);
        read_header_label(None, &mut back);
        assert_eq!(back.header_items, gregorian.header_items);
    }

    #[test]
    fn note_attributes_cross_a_syllable_label() {
        let syllable = Syllable {
            leading_attributes: vec!["[alt:Intro]".to_owned()],
            notes: vec![
                scoretk_core::model::GregorianNote {
                    letter: 'g',
                    attributes: vec![
                        scoretk_core::model::NoteAttribute {
                            after: 0,
                            text: "[cs:t]".to_owned(),
                        },
                        scoretk_core::model::NoteAttribute {
                            after: 2,
                            text: "[oh:h]".to_owned(),
                        },
                    ],
                    ..Default::default()
                },
                scoretk_core::model::GregorianNote {
                    letter: 'h',
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let label = attributes_label(&syllable).expect("a label");
        let mut back = Syllable {
            notes: syllable.notes.clone(),
            ..Default::default()
        };
        for note in &mut back.notes {
            note.attributes.clear();
        }
        read_attributes_label(Some(&label), &mut back);
        assert_eq!(back.leading_attributes, syllable.leading_attributes);
        assert_eq!(back.notes[0].attributes, syllable.notes[0].attributes);
        assert!(back.notes[1].attributes.is_empty());
    }
}
