// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// La syntaxe des annotations et des paroles suit la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>). Aucune ligne des
// scripts `xml2abc.py` et `abc2xml.py` de Willem Vree n'est copiée.
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

//! Échappement des textes ABC : annotations `"…"` et paroles `w:`.

/// Échappe un texte destiné à une annotation `"…"`.
///
/// ABC ne connaît pas d'échappement du guillemet double : il est remplacé par
/// une apostrophe. Les sauts de ligne deviennent des espaces, car une
/// annotation tient sur une ligne.
pub fn escape_annotation(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
        .replace('"', "'")
        .trim()
        .to_owned()
}

/// Échappe une syllabe de paroles.
///
/// Dans un champ `w:`, l'espace sépare les syllabes, `-` marque la césure et
/// `_` la prolongation. Un tiret littéral s'écrit `\-`, une espace littérale
/// `\ `, un antislash `\\`.
pub fn escape_lyric(text: &str) -> String {
    let mut result = String::new();
    for character in text.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '-' => result.push_str("\\-"),
            ' ' => result.push_str("\\ "),
            '\r' | '\n' | '\t' => result.push(' '),
            other => result.push(other),
        }
    }
    result
}

/// Décode une syllabe de paroles lue dans un champ `w:`.
pub fn unescape_lyric(text: &str) -> String {
    let mut result = String::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(escaped) = characters.next() {
                result.push(escaped);
            }
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotations() {
        assert_eq!(escape_annotation("Allegro \"vif\""), "Allegro 'vif'");
        assert_eq!(escape_annotation("deux\nlignes"), "deux lignes");
    }

    #[test]
    fn paroles() {
        assert_eq!(escape_lyric("hi-mid"), "hi\\-mid");
        assert_eq!(escape_lyric("a b"), "a\\ b");
        assert_eq!(unescape_lyric("hi\\-mid"), "hi-mid");
        assert_eq!(unescape_lyric("a\\ b"), "a b");
    }
}
