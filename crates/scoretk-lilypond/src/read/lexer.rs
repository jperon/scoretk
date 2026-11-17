// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage d'un sous-ensemble de `lily/lexer.ll` de LilyPond, Copyright (C)
// 1998--2026 Han-Wen Nienhuys et al., modifié (réécrit en Rust) en 2026.
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

//! Analyseur lexical du sous-ensemble LilyPond.
//!
//! Suit les états du lexer de LilyPond (`lily/lexer.ll`) : un mot nu
//! ([`Kind::Word`]) peut être un nom de note, une variable ou un symbole ;
//! `\nom` est une commande ([`Kind::Command`]) ; les nombres, fractions
//! (`3/2`), chaînes et ponctuations sont des jetons distincts. Les commentaires
//! `%` et `%{ … %}` sont retirés à la lecture, comme `longcomment` dans le
//! lexer amont.
//!
//! Le lexer ne connaît pas la langue des hauteurs : `cis` reste un mot, que
//! [`super::pitch`] interprète selon `\language`.

use std::fmt;

/// Nature d'un jeton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Mot nu : nom de note, variable, symbole de propriété.
    Word,
    /// Commande `\nom` (le nom est sans l'antislash).
    Command,
    /// Chaîne entre guillemets, sans les guillemets.
    Str,
    /// Nombre entier, décimal ou fraction `n/d`.
    Number,
    /// Ponctuation d'un caractère.
    Punct(char),
}

/// Jeton avec sa position (ligne, colonne).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// Nature du jeton.
    pub kind: Kind,
    /// Texte du jeton (contenu de la chaîne, nom de la commande…).
    pub text: String,
    /// Ligne, à partir de 1.
    pub line: u32,
    /// Colonne, à partir de 1.
    pub column: u32,
}

impl Token {
    /// Vrai pour un mot nu égal au texte donné.
    pub(crate) fn is_word(&self, word: &str) -> bool {
        self.kind == Kind::Word && self.text == word
    }

    /// Vrai pour une commande égale au nom donné (sans l'antislash).
    pub(crate) fn is_command(&self, name: &str) -> bool {
        self.kind == Kind::Command && self.text == name
    }

    /// Vrai pour la ponctuation donnée.
    pub(crate) fn is_punct(&self, character: char) -> bool {
        self.kind == Kind::Punct(character) && self.text == character.to_string()
    }

    /// Valeur entière du jeton, s'il est numérique et entier.
    pub(crate) fn as_integer(&self) -> Option<i64> {
        if self.kind != Kind::Number {
            return None;
        }
        self.text.parse().ok()
    }

    /// Valeur décimale du jeton, s'il est numérique et sans fraction.
    pub(crate) fn as_decimal(&self) -> Option<f64> {
        if self.kind != Kind::Number || self.text.contains('/') {
            return None;
        }
        self.text.parse().ok()
    }

    /// Fraction `num/den` ou entier du jeton.
    pub(crate) fn as_fraction(&self) -> Option<(i64, i64)> {
        if self.kind != Kind::Number {
            return None;
        }
        match self.text.split_once('/') {
            Some((numerator, denominator)) => {
                Some((numerator.parse().ok()?, denominator.parse().ok()?))
            }
            None => Some((self.text.parse().ok()?, 1)),
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::Word => write!(f, "{}", self.text),
            Kind::Command => write!(f, "\\{}", self.text),
            Kind::Str => write!(f, "\"{}\"", self.text),
            Kind::Number => write!(f, "{}", self.text),
            Kind::Punct(character) => write!(f, "{character}"),
        }
    }
}

/// Découpe une source LilyPond en jetons.
///
/// Les caractères inconnus sont conservés en ponctuation : le parseur les
/// ignore avec un avertissement plutôt que d'échouer.
pub(crate) fn tokenize(source: &str) -> Vec<Token> {
    let characters: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1u32;
    let mut column = 1u32;

    while index < characters.len() {
        let character = characters[index];
        let (token_line, token_column) = (line, column);

        // Espaces et retours à la ligne.
        if character == '\n' {
            index += 1;
            line += 1;
            column = 1;
            continue;
        }
        if character.is_whitespace() {
            index += 1;
            column += 1;
            continue;
        }

        // Commentaire de ligne.
        if character == '%' && characters.get(index + 1) != Some(&'{') {
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
                column += 1;
            }
            continue;
        }

        // Commentaire de bloc `%{ … %}`, imbricable comme `longcomment`.
        if character == '%' && characters.get(index + 1) == Some(&'{') {
            let mut depth = 0usize;
            while index < characters.len() {
                if characters[index] == '%' && characters.get(index + 1) == Some(&'{') {
                    depth += 1;
                    index += 2;
                    column += 2;
                    continue;
                }
                if characters[index] == '%' && characters.get(index + 1) == Some(&'}') {
                    depth -= 1;
                    index += 2;
                    column += 2;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                if characters[index] == '\n' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
                index += 1;
            }
            continue;
        }

        // Chaîne entre guillemets, avec échappements `\"` et `\\`.
        if character == '"' {
            index += 1;
            column += 1;
            let mut text = String::new();
            while index < characters.len() {
                let current = characters[index];
                if current == '\\' {
                    if let Some(next) = characters.get(index + 1) {
                        text.push(*next);
                        index += 2;
                        column += 2;
                        continue;
                    }
                }
                if current == '"' {
                    index += 1;
                    column += 1;
                    break;
                }
                if current == '\n' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
                text.push(current);
                index += 1;
            }
            tokens.push(Token {
                kind: Kind::Str,
                text,
                line: token_line,
                column: token_column,
            });
            continue;
        }

        // Commande `\nom` ou commande ponctuelle (`\!`, `\(`, `\\`…).
        if character == '\\' {
            index += 1;
            column += 1;
            let start = index;
            if characters.get(index).is_some_and(char::is_alphabetic) {
                while index < characters.len() && is_symbol_continue(&characters, index) {
                    index += 1;
                    column += 1;
                }
            } else if index < characters.len() {
                // Commande ponctuelle : `\!`, `\(`, `\\`, `\=`, `\<`…
                index += 1;
                column += 1;
            }
            let text: String = characters[start..index].iter().collect();
            tokens.push(Token {
                kind: Kind::Command,
                text,
                line: token_line,
                column: token_column,
            });
            continue;
        }

        // Nombre, fraction ou décimale.
        if character.is_ascii_digit() {
            let start = index;
            while index < characters.len() && characters[index].is_ascii_digit() {
                index += 1;
                column += 1;
            }
            // Fraction `n/d` ou décimale `n.d`, sans espace.
            if index < characters.len() && characters[index] == '/' {
                let slash = index;
                index += 1;
                column += 1;
                let denominator_start = index;
                while index < characters.len() && characters[index].is_ascii_digit() {
                    index += 1;
                    column += 1;
                }
                // Un `/` sans chiffre n'appartient pas au nombre.
                if index == denominator_start {
                    index = slash;
                    column -= 1;
                }
            } else if index < characters.len() && characters[index] == '.' {
                index += 1;
                column += 1;
                while index < characters.len() && characters[index].is_ascii_digit() {
                    index += 1;
                    column += 1;
                }
            }
            let text: String = characters[start..index].iter().collect();
            tokens.push(Token {
                kind: Kind::Number,
                text,
                line: token_line,
                column: token_column,
            });
            continue;
        }

        // Mot : lettre, puis lettres, chiffres et `-`/`_` internes
        // (`SYMBOL` du lexer amont).
        if character.is_alphabetic() {
            let start = index;
            while index < characters.len() && is_symbol_continue(&characters, index) {
                index += 1;
                column += 1;
            }
            let text: String = characters[start..index].iter().collect();
            tokens.push(Token {
                kind: Kind::Word,
                text,
                line: token_line,
                column: token_column,
            });
            continue;
        }

        // Ponctuation.
        index += 1;
        column += 1;
        tokens.push(Token {
            kind: Kind::Punct(character),
            text: character.to_string(),
            line: token_line,
            column: token_column,
        });
    }

    tokens
}

/// Vrai si le caractère à `index` continue un symbole.
///
/// Reprend `SYMBOL` de `lily/lexer.ll` : une lettre, un chiffre après la
/// première lettre, ou `-`/`_` suivi d'une lettre.
fn is_symbol_continue(characters: &[char], index: usize) -> bool {
    let Some(character) = characters.get(index) else {
        return false;
    };
    if character.is_alphabetic() || character.is_ascii_digit() {
        return true;
    }
    if *character == '-' || *character == '_' {
        return characters
            .get(index + 1)
            .is_some_and(|next| next.is_alphabetic());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str) -> Vec<String> {
        tokenize(source).iter().map(ToString::to_string).collect()
    }

    #[test]
    fn notes_et_durees() {
        assert_eq!(
            texts("c'4. d8~"),
            vec!["c", "'", "4", ".", "d", "8", "~"]
        );
    }

    #[test]
    fn fractions_et_commandes() {
        assert_eq!(
            texts("\\tuplet 3/2 { c8 } \\time 6/8"),
            vec![
                "\\tuplet",
                "3/2",
                "{",
                "c",
                "8",
                "}",
                "\\time",
                "6/8"
            ]
        );
    }

    #[test]
    fn commentaires() {
        assert_eq!(
            texts("c4 % commentaire\nd4 %{ bloc\n%{ imbriqué %} x %}\ne4"),
            vec!["c", "4", "d", "4", "e", "4"]
        );
    }

    #[test]
    fn chaines_et_ponctuations() {
        assert_eq!(
            texts("\\markup { \"a \\\"b\\\"\" } ^ _ -."),
            vec![
                "\\markup",
                "{",
                "\"a \"b\"\"",
                "}",
                "^",
                "_",
                "-",
                "."
            ]
        );
    }

    #[test]
    fn voix_et_liaisons() {
        assert_eq!(
            texts("<< c4 \\\\ d4 >> \\( c4 \\)"),
            vec!["<", "<", "c", "4", "\\\\", "d", "4", ">", ">", "\\(", "c", "4", "\\)"]
        );
    }
}
