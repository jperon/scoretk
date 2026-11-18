// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la fonction musicale `parallelMusic` de
// `ly/music-functions-init.ly` de LilyPond, Copyright (C) 1998--2026 Han-Wen
// Nienhuys et al., modifié (réécrit en Rust) en 2026.
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

//! Répartition de `\parallelMusic` entre plusieurs variables.
//!
//! `ly/music-functions-init.ly` : « Within @var{music}, parallel music
//! sequences are separated by @samp{|} characters. The sequences are assigned
//! to the LilyPond music identifiers provided in @var{voice-ids}. » La
//! fonction coupe la liste musicale aux `BarCheckEvent` et répartit les
//! tranches circulairement (`circular-list` des voix), en retirant les
//! contrôles de mesure qui terminent une voix pour permettre de finir hors
//! mesure.
//!
//! Le lecteur travaille sur les jetons : `parse_music` consomme un `|` sans le
//! garder, on découpe donc le flux avant de l'analyser. Les `|` d'un bloc
//! imbriqué (`{ … }`, `<< … >>`) ne séparent pas les voix : LilyPond, lui,
//! découpe aussi les éléments imbriqués, mais un contrôle de mesure imbriqué
//! est un cas de figure qui n'apparaît pas dans la pratique.

use crate::lexer::{Kind, Token};

/// Découpe un flux de jetons aux contrôles de mesure `|` de premier niveau.
///
/// Chaque tranche est rendue sans son `|` final ; la tranche vide qui suit un
/// dernier `|` n'est pas rendue, comme LilyPond retire les contrôles de mesure
/// en fin de voix. Un `|` en début de flux rend une première tranche vide, ce
/// qui décale la voix suivante exactement comme le fait LilyPond. Rend `None`
/// quand la musique ne contient aucun contrôle de mesure : LilyPond l'ignore
/// alors avec l'avertissement « ignoring parallel music without bar checks ».
pub(crate) fn split_at_bar_checks(tokens: &[Token]) -> Option<Vec<Vec<Token>>> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    let mut braces = 0usize;
    let mut angles = 0usize;
    let mut saw_bar_check = false;
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if braces == 0 && angles == 0 && token.is_punct('|') {
            segments.push(std::mem::take(&mut current));
            saw_bar_check = true;
            index += 1;
            continue;
        }
        // `<<` et `>>` comptent comme une seule paire : un accord `<…>` écrit
        // avec des chevrons simples ne change pas la profondeur.
        if braces == 0
            && token.is_punct('<')
            && tokens.get(index + 1).is_some_and(|next| next.is_punct('<'))
        {
            angles += 1;
            current.push(token.clone());
            current.extend(tokens.get(index + 1).cloned());
            index += 2;
            continue;
        }
        if braces == 0
            && token.is_punct('>')
            && tokens.get(index + 1).is_some_and(|next| next.is_punct('>'))
        {
            angles = angles.saturating_sub(1);
            current.push(token.clone());
            current.extend(tokens.get(index + 1).cloned());
            index += 2;
            continue;
        }
        match token.kind {
            Kind::Punct('{') => braces += 1,
            Kind::Punct('}') => braces = braces.saturating_sub(1),
            _ => {}
        }
        current.push(token.clone());
        index += 1;
    }
    if !current.is_empty() {
        segments.push(current);
    }
    saw_bar_check.then_some(segments)
}

/// Noms de voix d'une liste Scheme `#'(a b c)` (ou `#'("a" "b")`).
///
/// Rend une liste vide si le texte n'est pas une liste parenthésée.
pub(crate) fn voice_names(text: &str) -> Vec<String> {
    let Some(open) = text.find('(') else {
        return Vec::new();
    };
    let Some(close) = text.rfind(')') else {
        return Vec::new();
    };
    if close <= open {
        return Vec::new();
    }
    text[open + 1..close]
        .split_whitespace()
        .filter_map(|word| {
            let word = word.trim_matches('"');
            let valid = !word.is_empty()
                && word
                    .chars()
                    .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_'));
            valid.then(|| word.to_owned())
        })
        .collect()
}
