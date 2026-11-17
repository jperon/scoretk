// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Textes et chaînes LilyPond.

use mscz2xml::model::{Placement, RichText};

/// Échappe une chaîne pour un littéral LilyPond entre guillemets.
///
/// Les retours à la ligne sont remplacés par des espaces : un texte de portée
/// sur plusieurs lignes n'a pas d'équivalent direct en `\markup`, et un saut
/// de ligne dans une chaîne casserait la lisibilité du fichier.
pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' | '\r' | '\t' => escaped.push(' '),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Texte brut d'un [`RichText`], échappé pour LilyPond.
pub(crate) fn rich_text(text: &RichText) -> String {
    escape(text.plain())
}

/// `\markup { "…" }` pour un texte déjà échappé.
pub(crate) fn markup(escaped: &str) -> String {
    format!("\\markup {{ \"{escaped}\" }}")
}

/// Modificateur d'attache verticale (`^` au-dessus, `_` au-dessous).
pub(crate) const fn direction(placement: Placement) -> char {
    match placement {
        Placement::Above => '^',
        Placement::Below => '_',
    }
}

/// Attache un `\markup` à la note courante, du côté demandé.
pub(crate) fn attach_markup(text: &str, placement: Placement) -> String {
    format!("{}{}", direction(placement), markup(&escape(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echappement_des_chaines() {
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape("deux\nlignes"), "deux lignes");
        assert_eq!(markup("Tempo"), "\\markup { \"Tempo\" }");
    }
}
