// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : vocabulaires partagés par les deux sens du
// dialecte grégorien du *Notation Reference* §17.4 (annotations de note
// `^"…"`, commandes de neumes), réécrit en Rust en 2026.
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

//! Vocabulaires communs au dialecte grégorien LilyPond.
//!
//! Le *Notation Reference* §17.4.7 définit des commandes pour les neumes
//! carrés, mais pas pour les caractères d'espacement ou de fusion du GABC
//! (`/`, `!`, `*`, …). Une annotation de note standard `c'^"…"` (§"Text
//! annotations") porte ces faits avec des **noms lisibles** : `glyph:
//! augmentum spacing`, `attribute: [oh:h]`, `sign: ,`, `clef: c3`, `above:
//! …`. Aucun octet n'est encodé en hexadécimal et aucun préfixe privé n'est
//! employé ; un lecteur humain voit le nom du signe au-dessus de la note.

/// Nom lisible d'un caractère de glyphe GABC.
pub fn glyph_name(gabc: char) -> Option<&'static str> {
    match gabc {
        '.' => Some("augmentum"),
        '\'' => Some("ictus"),
        '_' => Some("episema"),
        'w' => Some("quilisma"),
        '/' => Some("spacing"),
        '!' => Some("zero-space"),
        '~' => Some("deminutum"),
        'v' => Some("virga"),
        'x' => Some("flat"),
        '*' => Some("asterisk"),
        '>' => Some("diminutum"),
        'o' => Some("oriscus"),
        '1' => Some("one"),
        '0' => Some("zero"),
        'y' => Some("natural"),
        'V' => Some("virga-reversa"),
        's' => Some("stropha"),
        'O' => Some("oriscus-scampus"),
        '2' => Some("two"),
        'r' => Some("cavum"),
        '`' => Some("virgula"),
        '-' => Some("hyphen"),
        '+' => Some("plus"),
        ':' => Some("colon"),
        '{' => Some("brace-open"),
        '}' => Some("brace-close"),
        '@' => Some("fusion"),
        '3' => Some("three"),
        '<' => Some("auctum"),
        'u' => Some("u"),
        '"' => Some("quote"),
        '†' => Some("dagger"),
        '#' => Some("sharp"),
        '=' => Some("linea"),
        _ => None,
    }
}

/// Caractère GABC d'un nom lisible, ou `None` si le mot est inconnu.
pub fn glyph_char(name: &str) -> Option<char> {
    const NAMES: &[char] = &[
        '.', '\'', '_', 'w', '/', '!', '~', 'v', 'x', '*', '>', 'o', '1', '0', 'y', 'V', 's',
        'O', '2', 'r', '`', '-', '+', ':', '{', '}', '@', '3', '<', 'u', '"', '†', '#', '=',
    ];
    NAMES
        .iter()
        .copied()
        .find(|&gabc| glyph_name(gabc) == Some(name))
}

/// Écrit le glyphe source sous forme de noms lisibles séparés par des espaces.
pub fn glyph_label(glyph: &str) -> String {
    glyph
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| {
            glyph_name(character).map_or_else(|| character.to_string(), str::to_owned)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Relit un glyphe à partir de noms lisibles.
pub fn glyph_from_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(|word| glyph_char(word).map_or_else(|| word.to_owned(), |gabc| gabc.to_string()))
        .collect()
}

/// Fabrique une annotation de note `^"…"` à partir de faits étiquetés.
pub fn annotation(facts: &[(&str, String)]) -> String {
    let content = facts
        .iter()
        // Une annotation est délimitée par des guillemets : un guillemet dans
        // une valeur devient une apostrophe, comme dans les textes ABC.
        .map(|(label, value)| format!("{label}: {}", value.replace('"', "'")))
        .collect::<Vec<_>>()
        .join(" | ");
    format!("^\"{content}\"")
}

/// Sépare une annotation `^"…"` en couples (étiquette, valeur).
pub fn parse_annotation(text: &str) -> Vec<(String, String)> {
    text.strip_prefix("^\"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(text)
        .split(" | ")
        .filter_map(|fact| fact.split_once(": "))
        .map(|(label, value)| (label.to_owned(), value.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_labels_are_readable_and_invertible() {
        assert_eq!(glyph_label("w./"), "quilisma augmentum spacing");
        assert_eq!(glyph_from_label("virga augmentum spacing"), "v./");
        for gabc in ['w', '/', '!', '*', 'x', '~', 'v', '†'] {
            assert_eq!(glyph_char(glyph_name(gabc).expect("nom")), Some(gabc));
        }
    }

    #[test]
    fn annotations_use_labels_not_codes() {
        let text = annotation(&[("glyph", glyph_label("w")), ("attribute", "[oh:h]".to_owned())]);
        assert_eq!(text, "^\"glyph: quilisma | attribute: [oh:h]\"");
        assert_eq!(
            parse_annotation(&text),
            vec![
                ("glyph".to_owned(), "quilisma".to_owned()),
                ("attribute".to_owned(), "[oh:h]".to_owned()),
            ]
        );
    }
}
