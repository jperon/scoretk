// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/textbase.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026 : écriture
// du balisage interne (`TextBase::write`), inverse de `TextBase::createBlocks`.
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

//! Nombres, fractions et balisage des textes riches.
//!
//! Un `.mscx` stocke les textes sous forme de balisage MuseScore
//! (`<b>`, `<i>`, `<font face="…">`, `<sym>…</sym>`). Le modèle les conserve
//! sous forme de fragments formatés ([`RichText`]) ; [`markup`] reconstruit un
//! balisage que `RichText::from_markup` relit à l'identique.

use scoretk_core::model::{Fraction, RichText, MUSICAL_TEXT_FONT};

use crate::xmlwriter::escape_text;

/// Formate un nombre comme MuseScore : représentation la plus courte qui
/// relit exactement la même valeur.
pub fn number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    format!("{value}")
}

/// Formate une fraction MuseScore (`"3/4"`).
pub fn fraction(value: Fraction) -> String {
    format!("{}/{}", value.numerator(), value.denominator())
}

/// Noms de symboles connus du modèle, indexés par point de code.
///
/// Inverse de `scoretk_core::model::symbol_character` : seuls les symboles que le
/// lecteur sait convertir peuvent apparaître dans un [`RichText`].
const SYMBOLS: &[(char, &str)] = &[
    (' ', "space"),
    ('\u{ECB7}', "metAugmentationDot"),
    ('\u{ECA0}', "metNoteDoubleWhole"),
    ('\u{ECA1}', "metNoteDoubleWholeSquare"),
    ('\u{ECA2}', "metNoteWhole"),
    ('\u{ECA3}', "metNoteHalfUp"),
    ('\u{ECA5}', "metNoteQuarterUp"),
    ('\u{ECA7}', "metNote8thUp"),
    ('\u{ECA9}', "metNote16thUp"),
    ('\u{ECAB}', "metNote32ndUp"),
    ('\u{ECAD}', "metNote64thUp"),
    ('\u{ECAF}', "metNote128thUp"),
    ('\u{ECB1}', "metNote256thUp"),
    ('\u{ECB3}', "metNote512thUp"),
    ('\u{ECB5}', "metNote1024thUp"),
];

/// Indice du `;` qui ferme une référence d'entité commençant à `start`.
///
/// Reconnaît `&nom;` et `&#…;` (`MScore` conserve les deux telles quelles) ;
/// renvoie `None` pour un `&` isolé.
fn entity_end(characters: &[char], start: usize) -> Option<usize> {
    let mut index = start + 1;
    let limit = (start + 12).min(characters.len());
    while index < limit {
        match characters[index] {
            ';' => return (index > start + 1).then_some(index),
            character if character.is_ascii_alphanumeric() || character == '#' => index += 1,
            _ => return None,
        }
    }
    None
}

/// Nom de symbole d'un caractère, s'il est connu.
fn symbol_name(character: char) -> Option<&'static str> {
    SYMBOLS
        .iter()
        .find(|(known, _)| *known == character)
        .map(|(_, name)| *name)
}

/// Reconstruit le balisage MuseScore d'un texte riche.
///
/// Chaque fragment est encadré de ses balises (`<b>…</b>`, `<i>…</i>`,
/// `<font …>…</font>`) : le lecteur les ferme comme MuseScore, et les formats
/// sont ainsi explicites plutôt que dépendants du style du texte. Un symbole
/// musical reconnu redevient `<sym>nom</sym>`.
pub fn markup(text: &RichText) -> String {
    let mut out = String::new();
    for fragment in text.fragments() {
        let format = &fragment.format;
        let mut opened: Vec<&str> = Vec::new();
        for (flag, tag) in [
            (format.bold, "b"),
            (format.italic, "i"),
            (format.underline, "u"),
            (format.strike, "s"),
        ] {
            if flag {
                out.push('<');
                out.push_str(tag);
                out.push('>');
                opened.push(tag);
            }
        }
        let musical = format.font_family.as_deref() == Some(MUSICAL_TEXT_FONT);
        if !musical {
            if let Some(family) = &format.font_family {
                out.push_str("<font face=\"");
                out.push_str(&escape_text(family));
                out.push_str("\">");
                opened.push("font");
            }
        }
        if let Some(size) = format.font_size {
            out.push_str(&format!("<font size=\"{}\">", number(size)));
            opened.push("font");
        }
        let characters: Vec<char> = fragment.text.chars().collect();
        let mut index = 0;
        while index < characters.len() {
            let character = characters[index];
            if musical {
                if let Some(name) = symbol_name(character) {
                    out.push_str("<sym>");
                    out.push_str(name);
                    out.push_str("</sym>");
                    index += 1;
                    continue;
                }
            }
            match character {
                // MuseScore conserve les entités du texte interne (`&quot;`)
                // et les écrit telles quelles : une entité valide reste brute,
                // un `&` isolé est échappé pour rester du XML valide.
                '&' => {
                    if let Some(end) = entity_end(&characters, index) {
                        out.extend(&characters[index..=end]);
                        index = end + 1;
                        continue;
                    }
                    out.push_str("&amp;");
                }
                '<' => out.push_str("&lt;"),
                other => out.push(other),
            }
            index += 1;
        }
        for tag in opened.into_iter().rev() {
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
    }
    out
}

/// Vérifie l'inverse de `symbol_character` (le test vit ici pour que les deux
/// tables restent synchronisées).
#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::symbol_character;

    #[test]
    fn symboles_inverses() {
        for (character, name) in SYMBOLS {
            assert_eq!(
                symbol_character(name),
                Some(*character),
                "symbole {name} incohérent avec la table du lecteur"
            );
            assert_eq!(symbol_name(*character), Some(*name));
        }
    }

    #[test]
    fn balisage_ecrit() {
        // Textes construits par `RichText::new` : le balisage est interprété,
        // puis réécrit avec les balises explicites du format.
        assert_eq!(markup(&RichText::new("Visible text")), "Visible text");
        assert_eq!(markup(&RichText::new("a<b>b</b>c")), "a<b>b</b>c");
        assert_eq!(
            markup(&RichText::new("<i>italique</i> droit")),
            "<i>italique</i> droit"
        );
        assert_eq!(
            markup(&RichText::new("<sym>metNoteQuarterUp</sym> = 80")),
            "<sym>metNoteQuarterUp</sym> = 80"
        );
        // Entités conservées telles quelles par le modèle (le balisage interne
        // de MuseScore garde `&quot;`), `&` non-entity échappé.
        assert_eq!(markup(&RichText::from_markup("a &amp; b")), "a &amp; b");
        assert_eq!(
            markup(&RichText::from_markup("&quot;plus&quot;")),
            "&quot;plus&quot;"
        );
    }

    #[test]
    fn nombres_et_fractions() {
        assert_eq!(number(1.75), "1.75");
        assert_eq!(number(0.0), "0");
        assert_eq!(number(f64::NAN), "0");
        assert_eq!(fraction(Fraction::new(3, 4)), "3/4");
        assert_eq!(fraction(Fraction::from_integer(2)), "2/1");
    }
}
