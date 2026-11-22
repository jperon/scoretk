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
//! Comme les règles `#` et `$` du lexer amont, qui passent la main au lecteur
//! Guile (`parse_embedded_scheme`), une expression Scheme est lue **d'un seul
//! tenant** et forme un unique jeton ([`Kind::Scheme`]) : listes imbriquées,
//! chaînes, caractères `#\(`, commentaires Scheme et musique embarquée
//! `#{ … #}` comprise. Le texte du jeton est le datum, sans le `#`/`$` initial.
//!
//! En revanche, un bloc `#{ … #}` (ou `$#{ … #}`) placé là où une valeur est
//! attendue n'est pas du Scheme : comme `embedded_lexer` dans le lexer amont,
//! son contenu est du LilyPond. Il forme un jeton [`Kind::Embedded`] dont le
//! texte est la source intérieure, que le parseur relit comme musique.
//!
//! Le lexer ne connaît pas la langue des hauteurs : `cis` reste un mot, que
//! [`crate::pitch`] interprète selon `\language`.

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
    /// Expression Scheme `#datum` ou `$datum`, texte = datum sans le préfixe.
    Scheme,
    /// Musique embarquée `#{ … #}` ou `$#{ … #}`, texte = source intérieure.
    Embedded,
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

    /// Vrai pour une expression Scheme (`#…`, `$…`).
    pub(crate) fn is_scheme(&self) -> bool {
        self.kind == Kind::Scheme
    }

    /// Valeur décimale d'une expression Scheme (`#120`, `#0.5`).
    pub(crate) fn scheme_decimal(&self) -> Option<f64> {
        if self.kind != Kind::Scheme {
            return None;
        }
        self.text.trim().parse().ok()
    }

    /// Fraction portée par une expression Scheme.
    ///
    /// Reconnaît `#3/4`, `#9/8`, `#(ly:make-moment 3/4)` (dernier atome) et la
    /// forme à deux entiers `#(ly:make-moment 3 4)`, fréquente dans les
    /// partitions réelles (`\set Timing.measureLength = #(ly:make-moment 1 8)`).
    /// La référence de notation §2.3.1 laisse le détail des moments au Scheme ;
    /// on suit la primitive `ly:make-moment` de `lily/moment-scheme.cc` (les
    /// deux entiers sont numérateur et dénominateur).
    pub(crate) fn scheme_fraction(&self) -> Option<(i64, i64)> {
        if self.kind != Kind::Scheme {
            return None;
        }
        let text = self.text.trim();
        let atom = if let Some(inner) = text
            .strip_prefix('(')
            .and_then(|part| part.strip_suffix(')'))
        {
            let words: Vec<&str> = inner.split_whitespace().filter(|w| !w.is_empty()).collect();
            // `(ly:make-moment n d)` : deux arguments entiers forment la
            // fraction ; avec un seul, c'est le dernier atome qui la porte.
            if words
                .first()
                .is_some_and(|word| word.contains("make-moment"))
                && words.len() >= 3
            {
                return Some((words[1].parse().ok()?, words[2].parse().ok()?));
            }
            *words.last()?
        } else {
            text
        };
        match atom.split_once('/') {
            Some((numerator, denominator)) => {
                Some((numerator.parse().ok()?, denominator.parse().ok()?))
            }
            None => Some((atom.parse().ok()?, 1)),
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
            // Le préfixe est toujours affiché `#`, y compris pour `$`.
            Kind::Scheme => write!(f, "#{}", self.text),
            Kind::Embedded => {
                write!(f, "#{{")?;
                write!(f, "{}", self.text)?;
                write!(f, "#}}")
            }
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

        // Musique embarquée `#{ … #}` (ou `$#{ … #}`) hors d'un datum Scheme :
        // le contenu est du LilyPond, on en fait un jeton pour que le parseur
        // le relise comme une expression musicale. Comme `embedded_lexer` dans
        // le lexer amont, les chaînes et commentaires LilyPond intérieurs ne
        // ferment pas le bloc.
        if (character == '#' && characters.get(index + 1) == Some(&'{'))
            || (character == '$'
                && characters.get(index + 1) == Some(&'#')
                && characters.get(index + 2) == Some(&'{'))
        {
            let open = if character == '$' { index + 1 } else { index };
            let (start, content_end, end) = embedded_music_span(&characters, open);
            let text: String = characters[start..content_end].iter().collect();
            advance_position(&characters, index, end, &mut line, &mut column);
            index = end;
            tokens.push(Token {
                kind: Kind::Embedded,
                text,
                line: token_line,
                column: token_column,
            });
            continue;
        }

        // Expression Scheme `#datum` / `$datum` : le lexer amont passe la main
        // au lecteur Guile (`parse_embedded_scheme`), qui lit un datum
        // complet. On en fait un seul jeton pour ne pas découper listes et
        // chaînes en mots.
        if character == '#' || character == '$' {
            let (start, end) = scheme_datum_span(&characters, index + 1);
            if end > start {
                let text: String = characters[start..end].iter().collect();
                advance_position(&characters, index, end, &mut line, &mut column);
                index = end;
                tokens.push(Token {
                    kind: Kind::Scheme,
                    text,
                    line: token_line,
                    column: token_column,
                });
                continue;
            }
            // Datum absent (`#}`, `#)`…) : ponctuation ordinaire.
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
            if characters
                .get(index)
                .is_some_and(|character| character.is_alphabetic())
            {
                while index < characters.len() && is_symbol_continue(&characters, index) {
                    index += 1;
                    column += 1;
                }
            } else if characters.get(index) == Some(&'"') {
                // Identifiant entre guillemets échappés : `\"violin1"`
                // (`lily/parser.yy` `identifier`), utilisé pour les noms
                // contenant des caractères arbitraires.
                index += 1;
                column += 1;
                while index < characters.len() && characters[index] != '"' {
                    index += 1;
                    column += 1;
                }
                if index < characters.len() {
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
            } else if index + 1 < characters.len()
                && characters[index] == '.'
                && characters[index + 1].is_ascii_digit()
            {
                // Décimale `1.5` seulement : `4.` est une durée pointée.
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
    if character.is_alphabetic() {
        return true;
    }
    if *character == '-' || *character == '_' {
        return characters
            .get(index + 1)
            .is_some_and(|next| next.is_alphabetic());
    }
    false
}

/// Caractères qui ne peuvent pas commencer un mot de paroles.
///
/// Repris de la règle `<lyrics>` de `lily/lexer.ll`
/// (`[^|*.=$#{}\"\\ \t\n\r\f0-9][^$#{}\"\\ \t\n\r\f0-9]*`), à laquelle
/// s'ajoutent `~` (l'élision sépare les mots pour le parseur), `-` et `_`
/// (césure et mélisme, lus comme des jetons à part) et les caractères de la
/// musique `<`, `>`, `&` : un mot de paroles ne commence pas par eux.
pub(crate) const MOT_DEBUT_INTERDIT: &[char] = &[
    '|', '*', '.', '=', '$', '#', '{', '}', '"', '\\', '~', '-', '_', '<', '>', '&',
];

/// Caractères qui ne peuvent pas continuer un mot de paroles.
///
/// Comme la suite de la règle amont, sans `$#{}\"\\` ni chiffre ; `.`, `,`,
/// `!`, `?`, `:` et `'` en font partie (`bon,`, `gne.`, `qu'il`).
pub(crate) const MOT_SUITE_INTERDITE: &[char] =
    &['$', '#', '{', '}', '"', '\\', '~', '<', '>', '&'];

/// Mot de paroles à partir du jeton `start`, comme l'état `lyrics` du lexer
/// amont : une suite de caractères non blancs, ponctuation collée comprise.
///
/// Dans le mot, `_` devient une espace (`lyric_fudge` amont) : `Ah_!` est une
/// seule syllabe « Ah ! ». Seul un `_` isolé entre blancs reste un mélisme ;
/// un mot n'est donc jamais réduit à des espaces (on retire ceux des bords,
/// sans effet au rendu). Les chiffres ne font pas partie du mot : le parseur
/// les lit comme la durée de la syllabe (`la4`), comme le lexer amont.
///
/// Renvoie le texte du mot et le nombre de jetons qu'il occupe.
pub(crate) fn mot_de_paroles(tokens: &[Token], start: usize) -> Option<(String, usize)> {
    let first = tokens.get(start)?;
    let mut text = match &first.kind {
        Kind::Word => first.text.clone(),
        Kind::Punct(character) if !MOT_DEBUT_INTERDIT.contains(character) => character.to_string(),
        _ => return None,
    };
    // Colonne de la fin du dernier jeton, pour exiger la contiguïté : deux
    // jetons séparés par un blanc sont deux syllabes.
    let mut end = first.column + first.text.chars().count() as u32;
    let mut consumed = 1;
    while let Some(next) = tokens.get(start + consumed) {
        if next.line != first.line || next.column != end {
            break;
        }
        let piece = match &next.kind {
            Kind::Word => next.text.clone(),
            Kind::Punct(character) if !MOT_SUITE_INTERDITE.contains(character) => {
                character.to_string()
            }
            // Chiffres (durée de la syllabe), chaînes, commandes : hors du mot.
            _ => break,
        };
        text.push_str(&piece);
        end = next.column + next.text.chars().count() as u32;
        consumed += 1;
    }
    Some((text.replace('_', " ").trim().to_owned(), consumed))
}

/// Met à jour ligne et colonne pour les caractères `from..to`.
fn advance_position(characters: &[char], from: usize, to: usize, line: &mut u32, column: &mut u32) {
    for character in &characters[from..to] {
        if *character == '\n' {
            *line += 1;
            *column = 1;
        } else {
            *column += 1;
        }
    }
}

/// Étendue du datum Scheme qui suit `#` ou `$`.
///
/// Renvoie l'indice du premier caractère du datum (après espaces et
/// commentaires) et celui qui le suit immédiatement. Un `end <= start` signale
/// un datum absent, à traiter comme une ponctuation ordinaire.
fn scheme_datum_span(characters: &[char], start: usize) -> (usize, usize) {
    let begin = skip_scheme_atmosphere(characters, start);
    let end = scheme_datum_end(characters, begin).max(begin);
    (begin, end)
}

/// Saute espaces, commentaires `;`, blocs `#| … |#` et commentaires de datum
/// `#;` avant un datum Scheme.
fn skip_scheme_atmosphere(characters: &[char], mut index: usize) -> usize {
    loop {
        while characters
            .get(index)
            .is_some_and(|character| character.is_whitespace())
        {
            index += 1;
        }
        if matches!(characters.get(index), Some(&';')) {
            while characters
                .get(index)
                .is_some_and(|character| *character != '\n')
            {
                index += 1;
            }
            continue;
        }
        if matches!(characters.get(index), Some(&'#'))
            && matches!(characters.get(index + 1), Some(&'|'))
        {
            index = skip_scheme_block_comment(characters, index);
            continue;
        }
        if matches!(characters.get(index), Some(&'#'))
            && matches!(characters.get(index + 1), Some(&';'))
        {
            index = scheme_datum_end(characters, index + 2);
            continue;
        }
        return index;
    }
}

/// Indice qui suit le datum Scheme commençant à `start`.
fn scheme_datum_end(characters: &[char], start: usize) -> usize {
    let index = skip_scheme_atmosphere(characters, start);
    let Some(character) = characters.get(index) else {
        return characters.len();
    };
    match character {
        '(' => scheme_list_end(characters, index),
        // Parenthèse orpheline : pas de datum, `#` restera ponctuation.
        ')' => index,
        '"' => scheme_string_end(characters, index),
        '\'' | '`' => scheme_datum_end(characters, index + 1),
        ',' => {
            let next = if matches!(characters.get(index + 1), Some(&'@')) {
                index + 2
            } else {
                index + 1
            };
            scheme_datum_end(characters, next)
        }
        '#' => match characters.get(index + 1) {
            // Vecteur `#(…)`.
            Some(&'(') => scheme_list_end(characters, index + 1),
            Some(&'\\') => scheme_character_end(characters, index),
            Some(&'{') => scheme_embedded_music_end(characters, index),
            // Commentaire seul : le datum commence après.
            Some(&'|') => skip_scheme_block_comment(characters, index),
            Some(&';') => scheme_datum_end(characters, index + 2),
            _ => scheme_atom_end(characters, index),
        },
        _ => scheme_atom_end(characters, index),
    }
}

/// Vrai pour un caractère qui termine un atome Scheme.
///
/// En plus des délimiteurs de Guile, `{` et `}` terminent un atome : `#}` hors
/// musique embarquée doit rester la ponctuation `#` puis `}`.
fn is_scheme_delimiter(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '(' | ')' | '"' | '\'' | '`' | ',' | ';' | '{' | '}'
        )
}

/// Indice qui suit l'atome Scheme commençant à `start`.
fn scheme_atom_end(characters: &[char], start: usize) -> usize {
    let mut index = start;
    while index < characters.len() && !is_scheme_delimiter(characters[index]) {
        index += 1;
    }
    index
}

/// Indice qui suit la liste `( … )` commençant à `start`.
///
/// Ignore les parenthèses des chaînes, des caractères `#\(` `#\)`, des
/// commentaires Scheme et de la musique embarquée `#{ … #}` : une liste
/// `#(ly:expect-warning (G_ "… (…)"))` reste d'un seul tenant.
fn scheme_list_end(characters: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < characters.len() {
        match characters[index] {
            '(' => {
                depth += 1;
                index += 1;
            }
            ')' => {
                depth = depth.saturating_sub(1);
                index += 1;
                if depth == 0 {
                    return index;
                }
            }
            '"' => index = scheme_string_end(characters, index),
            ';' => {
                while characters
                    .get(index)
                    .is_some_and(|character| *character != '\n')
                {
                    index += 1;
                }
            }
            '#' => match characters.get(index + 1) {
                Some(&'|') => index = skip_scheme_block_comment(characters, index),
                Some(&';') => index = scheme_datum_end(characters, index + 2),
                Some(&'\\') => index = scheme_character_end(characters, index),
                Some(&'{') => index = scheme_embedded_music_end(characters, index),
                _ => index += 1,
            },
            _ => index += 1,
        }
    }
    characters.len()
}

/// Indice qui suit la chaîne Scheme commençant à `start` (guillemet inclus).
fn scheme_string_end(characters: &[char], start: usize) -> usize {
    let mut index = start + 1;
    while index < characters.len() {
        match characters[index] {
            '\\' => index += 2,
            '"' => return index + 1,
            _ => index += 1,
        }
    }
    characters.len()
}

/// Indice qui suit le caractère Scheme `#\x`, `#\space` ou `#\(` commençant à
/// `start`.
fn scheme_character_end(characters: &[char], start: usize) -> usize {
    let mut index = start + 2;
    match characters.get(index) {
        Some(character) if character.is_alphanumeric() => {
            while characters
                .get(index)
                .is_some_and(|character| character.is_alphanumeric())
            {
                index += 1;
            }
        }
        // Caractère unique, parenthèse ou espace compris.
        Some(_) => index += 1,
        None => {}
    }
    index
}

/// Indice qui suit le bloc `#| … |#` commençant à `start` (imbricable).
fn skip_scheme_block_comment(characters: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < characters.len() {
        if characters[index] == '#' && matches!(characters.get(index + 1), Some(&'|')) {
            depth += 1;
            index += 2;
        } else if characters[index] == '|' && matches!(characters.get(index + 1), Some(&'#')) {
            depth = depth.saturating_sub(1);
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    characters.len()
}

/// Indice qui suit la musique embarquée `#{ … #}` commençant à `start`.
///
/// Les chaînes et commentaires LilyPond (`%` et `%{ … %}`) sont sautés pour
/// qu'un `#}` cité ne ferme pas le bloc ; `#{` imbriqué est compté.
fn scheme_embedded_music_end(characters: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < characters.len() {
        if characters[index] == '#' && matches!(characters.get(index + 1), Some(&'{')) {
            depth += 1;
            index += 2;
        } else if characters[index] == '#' && matches!(characters.get(index + 1), Some(&'}')) {
            depth = depth.saturating_sub(1);
            index += 2;
            if depth == 0 {
                return index;
            }
        } else if characters[index] == '"' {
            index = scheme_string_end(characters, index);
        } else if characters[index] == '%' {
            if matches!(characters.get(index + 1), Some(&'{')) {
                index = skip_lilypond_block_comment(characters, index);
            } else {
                while characters
                    .get(index)
                    .is_some_and(|character| *character != '\n')
                {
                    index += 1;
                }
            }
        } else {
            index += 1;
        }
    }
    characters.len()
}

/// Indice qui suit le commentaire LilyPond `%{ … %}` commençant à `start`
/// (imbricable, comme `longcomment`).
fn skip_lilypond_block_comment(characters: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < characters.len() {
        if characters[index] == '%' && matches!(characters.get(index + 1), Some(&'{')) {
            depth += 1;
            index += 2;
        } else if characters[index] == '%' && matches!(characters.get(index + 1), Some(&'}')) {
            depth = depth.saturating_sub(1);
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    characters.len()
}

/// Étendue du contenu de la musique embarquée ouverte à `open` (`#{`).
///
/// Renvoie le début du contenu, sa fin et l'indice qui suit le bloc. Sans
/// `#}` fermant, le contenu va jusqu'à la fin de la source.
fn embedded_music_span(characters: &[char], open: usize) -> (usize, usize, usize) {
    let start = (open + 2).min(characters.len());
    let end = scheme_embedded_music_end(characters, open);
    let closed = end >= start + 2 && characters[end - 2] == '#' && characters[end - 1] == '}';
    let content_end = if closed { end - 2 } else { end };
    (start, content_end, end)
}

/// Source intérieure du premier bloc `#{ … #}` d'un datum Scheme.
///
/// Sert au parseur pour reconnaître le corps musical d'une
/// `define-music-function` ; renvoie `None` si le datum n'en contient pas.
pub(crate) fn embedded_music_body(source: &str) -> Option<String> {
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        match characters[index] {
            '"' => index = scheme_string_end(&characters, index),
            ';' => {
                while characters
                    .get(index)
                    .is_some_and(|character| *character != '\n')
                {
                    index += 1;
                }
            }
            '#' => match characters.get(index + 1) {
                Some(&'|') => index = skip_scheme_block_comment(&characters, index),
                Some(&'{') => {
                    let (start, end, _) = embedded_music_span(&characters, index);
                    return Some(characters[start..end].iter().collect());
                }
                _ => index += 1,
            },
            _ => index += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str) -> Vec<String> {
        tokenize(source).iter().map(ToString::to_string).collect()
    }

    #[test]
    fn notes_et_durees() {
        assert_eq!(texts("c'4. d8~"), vec!["c", "'", "4", ".", "d", "8", "~"]);
    }

    #[test]
    fn fractions_et_commandes() {
        assert_eq!(
            texts("\\tuplet 3/2 { c8 } \\time 6/8"),
            vec!["\\tuplet", "3/2", "{", "c", "8", "}", "\\time", "6/8"]
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
            vec!["\\markup", "{", "\"a \"b\"\"", "}", "^", "_", "-", "."]
        );
    }

    #[test]
    fn voix_et_liaisons() {
        assert_eq!(
            texts("<< c4 \\\\ d4 >> \\( c4 \\)"),
            vec!["<", "<", "c", "4", "\\\\", "d", "4", ">", ">", "\\(", "c", "4", "\\)"]
        );
    }

    #[test]
    fn expressions_scheme_d_un_seul_jeton() {
        // Une liste complète ne doit pas être découpée en mots.
        assert_eq!(
            texts("#(ly:set-option 'warning-as-error #t) c'4"),
            vec!["#(ly:set-option 'warning-as-error #t)", "c", "'", "4"]
        );
        // Booléens, vecteurs, nombres, symboles et quote.
        assert_eq!(
            texts("##t #f ##(1 2) #-1 #0.5 #9/8 #UP"),
            vec!["##t", "#f", "##(1 2)", "#-1", "#0.5", "#9/8", "#UP"]
        );
        assert_eq!(texts("#`x #,y #,@z"), vec!["#`x", "#,y", "#,@z"]);
        assert_eq!(
            texts("#'staff-space #' (1 . 2)"),
            vec!["#'staff-space", "#' (1 . 2)"]
        );
    }

    #[test]
    fn scheme_chaines_caracteres_et_commentaires() {
        let tokens = tokenize(
            "#(list \"chaîne ) avec parenthèse\" #\\) #\\( #\\space ; ) en commentaire\n 2)",
        );
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, Kind::Scheme);
        assert_eq!(
            tokens[0].text,
            "(list \"chaîne ) avec parenthèse\" #\\) #\\( #\\space ; ) en commentaire\n 2)"
        );
        // Bloc `#| … |#`, imbricable.
        let tokens = tokenize("#(a #| ) ( #| |# |# b)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "(a #| ) ( #| |# |# b)");
        // Commentaire de datum `#;`.
        let tokens = tokenize("#(a #;(b c) d)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "(a #;(b c) d)");
    }

    #[test]
    fn scheme_musique_embarquee_et_ponctuation() {
        // `#{ … #}` dans un datum Scheme reste un seul jeton.
        assert_eq!(
            texts("#(define m #{ c'4 d' #}) e4"),
            vec!["#(define m #{ c'4 d' #})", "e", "4"]
        );
        // Hors Scheme, `#{ … #}` est de la musique : un jeton à relire.
        assert_eq!(texts("#{ c'4 #}"), vec!["#{ c'4 #}"]);
        assert_eq!(texts("$#{ c'4 #} e4"), vec!["#{ c'4 #}", "e", "4"]);
        // Les chaînes et commentaires intérieurs ne ferment pas le bloc.
        assert_eq!(
            texts("#{ c'4-\"(#)\" % #}\n d'4 %{%} #}"),
            vec!["#{ c'4-\"(#)\" % #}\n d'4 %{%} #}"]
        );
        // Un `#` sans datum reste une ponctuation.
        assert_eq!(texts("#} #)"), vec!["#", "}", "#", ")"]);
    }

    #[test]
    fn scheme_immediat_dollar() {
        let tokens = tokenize("$(make-moment 1/4) d4");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, Kind::Scheme);
        assert_eq!(tokens[0].text, "(make-moment 1/4)");
        assert_eq!(tokens[2].line, 1);
    }
}
