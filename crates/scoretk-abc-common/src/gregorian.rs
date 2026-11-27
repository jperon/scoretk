// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : vocabulaires ABC 2.1 portant les faits du chant
// grégorien (§4.1/§4.6 hauteur et clé, §4.14 décorations, §4.16 symboles
// redéfinissables, §5 paroles), réécrit en Rust en 2026.
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

//! Porteurs ABC 2.1 des faits grégoriens, partagés par lecture et écriture.
//!
//! Un glyphe GABC est porté par une **décoration lisible** par caractère :
//! `!quilisma!`, `!spacing!`, `!virga!`, … (ABC 2.1 §4.14 autorise des noms
//! de symboles définis par l'application). Les quatre signes les plus
//! fréquents ont en plus une définition `U:` (§4.16) vers une décoration
//! standard (`!fermata!`, `!accent!`, `!tenuto!`, `!uppermordent!`) et sont
//! écrits avec leur raccourci `H`, `L`, `M`, `P`. Aucun code hexadécimal, aucun
//! préfixe privé : le document ABC reste lisible par un tiers.
//!
//! Les paroles vont dans les champs `w:` (§5), la clé dans le paramètre
//! `clef=` du champ `K:` (§4.6) et le titre dans `T:` (§3.1). Une annotation
//! `"^…"` dont le texte est `letter: …`, `glyph: …` ou `syllable-text: …`
//! serait un vidage de la source : elle n'est ni écrite ni lue.

/// Un caractère de glyphe GABC et son porteur ABC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphSymbol {
    /// Caractère source (`doc/Gabc.tex:452-700`).
    pub gabc: char,
    /// Nom de la décoration `!nom!` (§4.14).
    pub name: &'static str,
    /// Raccourci défini par `U:` (§4.16), s'il existe.
    pub shorthand: Option<char>,
    /// Décoration standard à laquelle le raccourci est assigné.
    pub standard: &'static str,
}

/// Table des glyphes présents dans le lot GABC, avec un nom signifiant.
///
/// Les noms suivent la terminologie de `Gabc.tex:452-700` (neumes, signes
/// rythmiques, espacements) ; les raccourcis `H`–`W` sont ceux que §4.16
/// autorise à redéfinir.
pub const GLYPH_SYMBOLS: &[GlyphSymbol] = &[
    GlyphSymbol {
        gabc: '.',
        name: "augmentum",
        shorthand: Some('H'),
        standard: "fermata",
    },
    GlyphSymbol {
        gabc: '\'',
        name: "ictus",
        shorthand: Some('L'),
        standard: "accent",
    },
    GlyphSymbol {
        gabc: '_',
        name: "episema",
        shorthand: Some('M'),
        standard: "tenuto",
    },
    GlyphSymbol {
        gabc: 'w',
        name: "quilisma",
        shorthand: Some('P'),
        standard: "uppermordent",
    },
    GlyphSymbol {
        gabc: '/',
        name: "spacing",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '!',
        name: "zero-space",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '~',
        name: "deminutum",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'v',
        name: "virga",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'x',
        name: "flat",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '*',
        name: "star",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '>',
        name: "diminutum",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'o',
        name: "oriscus",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '1',
        name: "one",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '0',
        name: "zero",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'y',
        name: "natural",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'V',
        name: "virga-reversa",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 's',
        name: "stropha",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'O',
        name: "oriscus-scampus",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '2',
        name: "two",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'r',
        name: "cavum",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '`',
        name: "virgula",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '-',
        name: "hyphen",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '+',
        name: "plus",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: ':',
        name: "colon",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '{',
        name: "brace-open",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '}',
        name: "brace-close",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '@',
        name: "fusion",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '3',
        name: "three",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '<',
        name: "augmentum-liquescens",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: 'u',
        name: "u",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '"',
        name: "quote",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '†',
        name: "dagger",
        shorthand: None,
        standard: "",
    },
    GlyphSymbol {
        gabc: '#',
        name: "sharp",
        shorthand: None,
        standard: "",
    },
];

/// Marqueur `I:` (§3.1) signalant un document ABC portant un chant grégorien.
pub const CHANT_MARKER: &str = "I:chant=gregorian";

/// Champ `K:` (§4.6) portant la clé GABC sous forme de clé ABC native.
///
/// `c3` → `K:C clef=C3`, `f4` → `K:C clef=F4` : le paramètre `clef=<lettre><ligne>`
/// est défini par ABC 2.1, contrairement à une copie du jeton source dans `I:`.
pub fn clef_field(clef: &str) -> String {
    let first = clef.split('@').next().unwrap_or(clef);
    let family = first.chars().next().unwrap_or('c').to_ascii_uppercase();
    let line = first
        .chars()
        .last()
        .and_then(|c| c.to_digit(10))
        .unwrap_or(4);
    format!("K:C clef={family}{line}")
}

/// Clé GABC relue depuis un champ `K:` (§4.6), ou `None`.
pub fn clef_from_field(line: &str) -> Option<String> {
    let value = line.strip_prefix("K:")?;
    for part in value.split_whitespace() {
        let Some(raw) = part.strip_prefix("clef=") else {
            continue;
        };
        let mut chars = raw.chars();
        let family = chars.next()?.to_ascii_lowercase();
        let line = chars.as_str().parse::<u8>().ok()?;
        if matches!(family, 'c' | 'f') && (1..=5).contains(&line) {
            return Some(format!("{family}{line}"));
        }
    }
    None
}

#[cfg(test)]
mod clef_tests {
    use super::*;

    #[test]
    fn clef_uses_the_native_k_parameter() {
        assert_eq!(clef_field("c3"), "K:C clef=C3");
        assert_eq!(clef_field("f4"), "K:C clef=F4");
        assert_eq!(clef_from_field("K:C clef=C3"), Some("c3".to_owned()));
        assert_eq!(clef_from_field("K:C"), None);
    }
}

/// Décoration d'un custos (`Gabc.tex:872-906`), avec son orthographe source.
pub fn custos_symbol(raw: &str) -> &'static str {
    match raw {
        "z" => "custos",
        "Z" => "custos-up",
        "z0" => "custos-zero",
        "Z0" => "custos-up-zero",
        _ => "custos",
    }
}

/// Caractère GABC d'une décoration ou d'un raccourci ABC.
pub fn glyph_char(token: &str) -> Option<char> {
    let name = token
        .strip_prefix('!')
        .and_then(|rest| rest.strip_suffix('!'));
    let looked_up = name.or(if token.len() == 1 { Some(token) } else { None })?;
    GLYPH_SYMBOLS
        .iter()
        .find(|symbol| {
            symbol.name == looked_up
                || symbol
                    .shorthand
                    .is_some_and(|short| short.to_string() == looked_up)
        })
        .map(|symbol| symbol.gabc)
}

/// Nom de la décoration portant un caractère de glyphe GABC.
pub fn glyph_name(gabc: char) -> Option<&'static str> {
    GLYPH_SYMBOLS
        .iter()
        .find(|symbol| symbol.gabc == gabc)
        .map(|symbol| symbol.name)
}

/// Jeton ABC portant un caractère de glyphe GABC.
pub fn glyph_token(gabc: char) -> String {
    let symbol = GLYPH_SYMBOLS
        .iter()
        .find(|symbol| symbol.gabc == gabc)
        .expect("glyphe de la table");
    symbol
        .shorthand
        .map_or_else(|| format!("!{}!", symbol.name), |short| short.to_string())
}

/// Lignes `U:` (§4.16) des raccourcis utilisés pour les quatre signes forts.
pub fn definitions() -> Vec<String> {
    GLYPH_SYMBOLS
        .iter()
        .filter(|symbol| symbol.shorthand.is_some())
        .map(|symbol| {
            format!(
                "U: {} = !{}!",
                symbol.shorthand.expect("raccourci"),
                symbol.standard
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_readable_and_invertible() {
        assert_eq!(glyph_token('w'), "P");
        assert_eq!(glyph_token('/'), "!spacing!");
        for symbol in GLYPH_SYMBOLS {
            assert_eq!(glyph_char(&glyph_token(symbol.gabc)), Some(symbol.gabc));
        }
        assert_eq!(glyph_char("!quilisma!"), Some('w'));
    }

    #[test]
    fn definitions_use_standard_decorations() {
        let lines = definitions();
        assert!(lines.contains(&"U: H = !fermata!".to_owned()));
        assert!(lines.contains(&"U: P = !uppermordent!".to_owned()));
    }
}
