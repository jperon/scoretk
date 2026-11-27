// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : libellés MEI 5.1 lisibles portant les faits
// grégoriens que le module NEUMES ne nomme pas, réécrit en Rust en 2026.
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

//! Libellés partagés entre l'écriture et la lecture MEI du chant grégorien.
//!
//! `<annot>` est un élément MEI 5.1 standard : son texte est lisible par un
//! tiers. Les faits non couverts par `<nc @pname @oct>` et les modificateurs
//! du module NEUMES (§6.4) y sont écrits avec des **noms de neumes**
//! (`glyph: virga augmentum`), jamais sous forme de code hexadécimal ni avec
//! un préfixe `gabc`.

use scoretk_core::model::GregorianNote;

/// Type d'une annotation MEI lisible portant les faits d'une note.
pub const NOTE_TYPE: &str = "chantNote";

/// Nom lisible d'un caractère de glyphe GABC (`Gabc.tex:452-700`).
pub fn glyph_name(character: char) -> &'static str {
    match character {
        '.' => "augmentum",
        '\'' => "ictus",
        '_' => "episema",
        'w' => "quilisma",
        '/' => "spacing",
        '!' => "zero-space",
        '~' => "deminutum",
        'v' => "virga",
        'x' => "flat",
        '*' => "asterisk",
        '>' => "diminutum",
        'o' => "oriscus",
        '1' => "one",
        '0' => "zero",
        'y' => "natural",
        'V' => "virga-reversa",
        's' => "stropha",
        'O' => "oriscus-scampus",
        '2' => "two",
        'r' => "cavum",
        '`' => "virgula",
        '-' => "hyphen",
        '+' => "plus",
        ':' => "colon",
        '{' => "brace-open",
        '}' => "brace-close",
        '@' => "fusion",
        '3' => "three",
        '<' => "auctum",
        'u' => "u",
        '"' => "quote",
        '†' => "dagger",
        '#' => "sharp",
        '=' => "linea",
        _ => "sign",
    }
}

/// Orthographe exacte d'un glyphe sous forme de noms séparés par des espaces.
pub fn glyph_label(glyph: &str) -> String {
    glyph
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| glyph_name(character).to_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Caractère GABC d'un nom lisible.
fn glyph_char(name: &str) -> Option<char> {
    const NAMES: &[(&str, char)] = &[
        ("augmentum", '.'),
        ("ictus", '\''),
        ("episema", '_'),
        ("quilisma", 'w'),
        ("spacing", '/'),
        ("zero-space", '!'),
        ("deminutum", '~'),
        ("virga", 'v'),
        ("flat", 'x'),
        ("asterisk", '*'),
        ("diminutum", '>'),
        ("oriscus", 'o'),
        ("one", '1'),
        ("zero", '0'),
        ("natural", 'y'),
        ("virga-reversa", 'V'),
        ("stropha", 's'),
        ("oriscus-scampus", 'O'),
        ("two", '2'),
        ("cavum", 'r'),
        ("virgula", '`'),
        ("hyphen", '-'),
        ("plus", '+'),
        ("colon", ':'),
        ("brace-open", '{'),
        ("brace-close", '}'),
        ("fusion", '@'),
        ("three", '3'),
        ("auctum", '<'),
        ("u", 'u'),
        ("quote", '"'),
        ("dagger", '†'),
        ("sharp", '#'),
        ("linea", '='),
    ];
    NAMES
        .iter()
        .find(|(label, _)| *label == name)
        .map(|(_, gabc)| *gabc)
}

/// Relit un glyphe écrit en noms lisibles.
pub fn glyph_from_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(|word| glyph_char(word).map_or_else(|| word.to_owned(), |gabc| gabc.to_string()))
        .collect()
}

/// Annotation lisible d'une note : lettre, glyphe et attributs crochetés.
pub fn note_label(note: &GregorianNote) -> String {
    let mut facts = vec![
        format!("letter: {}", note.letter),
        format!("glyph: {}", glyph_label(&note.glyph)),
    ];
    for attribute in &note.attributes {
        facts.push(format!("attribute: {}", attribute.text));
    }
    facts.join(" | ")
}

/// Faits exacts relus d'une annotation de note.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteLabel {
    pub letter: Option<char>,
    pub glyph: Option<String>,
    pub attributes: Vec<String>,
}

/// Relit une annotation écrite par [`note_label`].
pub fn parse_note_label(text: &str) -> NoteLabel {
    let mut result = NoteLabel::default();
    for fact in text.split(" | ") {
        let Some((label, value)) = fact.split_once(": ") else {
            continue;
        };
        match label {
            "letter" => result.letter = value.chars().next(),
            "glyph" => result.glyph = Some(glyph_from_label(value)),
            "attribute" => result.attributes.push(value.to_owned()),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_readable_and_invertible() {
        let note = GregorianNote {
            letter: 'g',
            glyph: "w./".to_owned(),
            ..GregorianNote::default()
        };
        let label = note_label(&note);
        assert_eq!(label, "letter: g | glyph: quilisma augmentum spacing");
        let read = parse_note_label(&label);
        assert_eq!(read.letter, Some('g'));
        assert_eq!(read.glyph.as_deref(), Some("w./"));
    }
}
