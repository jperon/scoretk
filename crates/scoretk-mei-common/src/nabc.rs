// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : table de correspondance entre les descripteurs
// NABC de Gregorio (`doc/GregorioNabcRef.tex`, GPL-3.0-or-later, commit
// `b63f074`) et les éléments du module NEUMES de MEI 5.1 (*Guidelines* ch. 6,
// §6.1, §6.1.1, §6.4.1–§6.4.9 ; schéma `mei-all-5.1.rng`, ECL-2.0), rédigé
// en 2026. Les règles NABC sont lues comme spécification et réécrites en
// Rust, sans incorporer de source C++.
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

//! Décomposition d'un snippet NABC en neumes MEI 5.1 structurés.
//!
//! Un snippet NABC est une suite de *descripteurs de neumes complexes*
//! (`GregorioNabcRef.tex:174-199`) : espacement horizontal, glyphes simples
//! reliés par `!`, sous-ponctis/préponctis, lettres significatives. Chaque
//! glyphe simple a un nom, des modificateurs et une hauteur rough (`h` +
//! lettre GABC, défaut `hf`). La table ci-dessous suit le chapitre 6 des
//! *Guidelines* : `<neume type>` porte le nom du glyphe (RNG `att.neumeType/
//! @type`, NMTOKEN libre), ses `<nc>` sont **sans hauteur** (notation
//! adiastématique) et portent `@intm` pour la direction mélodique
//! (§6.4.2, RNG `mei_att.intervalMelodic.attribute.intm`), les modificateurs
//! ont un élément dédié quand MEI en a un (§6.4.4, §6.4.5, §6.4.7), et les
//! lettres significatives passent par `<signifLet @place>` (§6.4.8).
//!
//! Ce module ne vise pas l'équivalence sémantique universelle : les *Guidelines*
//! §6.3 rappellent que des signes de même forme dans des styles régionaux
//! différents peuvent avoir des sens différents. Le texte NABC exact reste
//! donc toujours transporté à côté (`<signifLet type="gabc-nabc">`) et sert
//! à la lecture inverse ; les éléments structurés sont le regard que MEI
//! offre à un lecteur tiers.

/// Glyphe simple NABC et son porteur MEI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// Code NABC source à deux lettres (`vi`, `ql`, …).
    pub code: String,
    /// Nom MEI du type de neume (`att.neumeType/@type`).
    pub name: &'static str,
    /// Nombre de `<nc>` à émettre (1 pour un quilisma, porté par `@waves`).
    pub components: usize,
    /// Modificateurs NABC exacts (`S`, `G`, `M`, `-`, `>`, `~`, chiffres).
    pub modifiers: String,
    /// Hauteur NABC (`h` + lettre), ou `None` pour le défaut `hf`.
    pub height: Option<char>,
}

/// Un descripteur de neume complexe : un ou plusieurs glyphes, leurs
/// sous-ponctis/préponctis et leurs lettres significatives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    /// Caractères d'espacement qui précèdent le descripteur (`/`, `` ` ``).
    pub spacing: String,
    /// Glyphes simples, dans l'ordre source.
    pub glyphs: Vec<Glyph>,
    /// Sous-ponctis / préponctis, avec leur direction (`u` / `d`) et leur
    /// nombre de répétitions.
    pub puncta: Vec<(bool, usize)>,
    /// Lettres significatives : (raccourci, position 1–9).
    pub letters: Vec<(String, u8)>,
    /// Texte source exact du descripteur, conservé tel quel.
    pub raw: String,
}

/// Glyphe de base → (`@type` MEI, nombre de `<nc>`).
///
/// `GregorioNabcRef.tex:207-240` (St. Gall) et `:402-420` (Laon). Les noms
/// MEI suivent la terminologie des *Guidelines* §6.1.1 et de la table de
/// Cardine reproduite dans la référence NABC ; la RNG admet tout NMTOKEN.
fn basic(code: &str) -> Option<(&'static str, usize)> {
    Some(match code {
        "vi" => ("virga", 1),
        "pu" => ("punctum", 1),
        "ta" => ("tractulus", 1),
        "gr" => ("gravis", 1),
        "cl" => ("clivis", 2),
        "pe" => ("pes", 2),
        "po" => ("porrectus", 3),
        "to" => ("torculus", 3),
        "ci" => ("climacus", 3),
        "sc" => ("scandicus", 3),
        "pf" => ("porrectus-flexus", 4),
        "sf" => ("scandicus-flexus", 4),
        "tr" => ("torculus-resupinus", 4),
        "st" => ("stropha", 1),
        "ds" => ("distropha", 2),
        "ts" => ("tristropha", 3),
        "tg" => ("trigonus", 3),
        "bv" => ("bivirga", 2),
        "tv" => ("trivirga", 3),
        "pr" => ("pressus-maior", 2),
        "pi" => ("pressus-minor", 2),
        "vs" => ("virga-strata", 1),
        "or" => ("oriscus", 1),
        "sa" => ("salicus", 3),
        "pq" => ("pes-quassus", 2),
        "ql" => ("quilisma", 1),
        "qi" => ("quilisma", 1),
        "pt" => ("pes-stratus", 2),
        "ni" => ("nihil", 1),
        "un" => ("uncinus", 1),
        "oc" => ("oriscus-clivis", 2),
        _ => return None,
    })
}

/// Nom MEI → code NABC à deux lettres, pour relire un encodage tiers qui
/// n'aurait pas de porteur texte. Récriproque de [`basic`] (table :207-240,
/// :402-420). Les variantes `S`/`G`/`M` et les chiffres restent dans le texte
/// du snippet quand il est présent, sinon ils sont perdus et signalés.
pub fn base_code(name: &str) -> Option<&'static str> {
    Some(match name {
        "virga" => "vi",
        "punctum" => "pu",
        "tractulus" => "ta",
        "gravis" => "gr",
        "clivis" => "cl",
        "pes" => "pe",
        "porrectus" => "po",
        "torculus" => "to",
        "climacus" => "ci",
        "scandicus" => "sc",
        "porrectus-flexus" => "pf",
        "scandicus-flexus" => "sf",
        "torculus-resupinus" => "tr",
        "stropha" => "st",
        "distropha" => "ds",
        "tristropha" => "ts",
        "trigonus" => "tg",
        "bivirga" => "bv",
        "trivirga" => "tv",
        "pressus-maior" => "pr",
        "pressus-minor" => "pi",
        "virga-strata" => "vs",
        "oriscus" => "or",
        "salicus" => "sa",
        "pes-quassus" => "pq",
        "quilisma" => "ql",
        "pes-stratus" => "pt",
        "nihil" => "ni",
        "uncinus" => "un",
        "oriscus-clivis" => "oc",
        _ => return None,
    })
}

/// Vrai si le glyphe est un quilisma (`GregorioNabcRef.tex:235-236,433`) :
/// MEI le porte par `<quilisma @waves>` (§6.4.7) et non par plusieurs `<nc>`.
pub fn is_quilisma(name: &str) -> bool {
    name == "quilisma"
}

/// Nombre de crests d'un quilisma NABC (`ql` = 3, `qi` = 2).
pub fn quilisma_waves(code: &str) -> u8 {
    if code == "qi" {
        2
    } else {
        3
    }
}

/// Hauteur NABC → direction mélodique MEI (`@intm`, §6.4.2).
///
/// Les hauteurs NABC utilisent les mêmes lettres que GABC (`a`…`n`, `p`) et
/// le défaut est `hf` (`GregorioNabcRef.tex:258-269`). MEI n'a pas de portée
/// ici : seule la direction entre deux hauteurssuccessives est structurée.
pub fn interval(previous: char, current: char) -> &'static str {
    let height = |letter: char| {
        let low = letter.to_ascii_lowercase();
        (low as i32) - ('a' as i32) + 3 + i32::from(low > 'o')
    };
    match height(current).cmp(&height(previous)) {
        std::cmp::Ordering::Greater => "u",
        std::cmp::Ordering::Less => "d",
        std::cmp::Ordering::Equal => "s",
    }
}

/// Position d'une lettre significative → `@place` MEI (`data.EVENTREL`,
/// RNG :6466). La position 5 (Laon, « inside », `:449`) n'a pas de valeur
/// MEI : le chiffre reste dans le texte de l'élément et aucun `@place` n'est
/// émis ; les autres positions suivent la liste de `:300-312`.
pub fn place(digit: u8) -> Option<&'static str> {
    Some(match digit {
        1 => "above-left",
        2 => "above",
        3 => "above-right",
        4 => "left",
        6 => "right",
        7 => "below-left",
        8 => "below",
        9 => "below-right",
        _ => return None,
    })
}

fn is_modifier(char: char) -> bool {
    matches!(char, 'S' | 'G' | 'M' | '-' | '>' | '~' | '0'..='9')
}

fn is_modifier_letter(char: char) -> bool {
    matches!(char, 't' | 'u' | 'v' | 'w' | 'x' | 'y' | 'n' | 'q' | 'z')
}

/// Découpe un snippet NABC en descripteurs, sans rien inventer : les suites
/// non reconnues restent dans `raw` et sont transportées par le `<signifLet>`
/// de round trip.
pub fn parse(snippet: &str) -> Vec<Descriptor> {
    let chars: Vec<char> = snippet.chars().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        let mut spacing = String::new();
        while index < chars.len() && matches!(chars[index], '/' | '`') {
            spacing.push(chars[index]);
            index += 1;
        }
        let mut glyphs = Vec::new();
        loop {
            if index + 1 >= chars.len() {
                break;
            }
            let code: String = chars[index..index + 2].iter().collect();
            let Some((name, components)) = basic(&code) else {
                break;
            };
            index += 2;
            let mut modifiers = String::new();
            while index < chars.len() && is_modifier(chars[index]) {
                modifiers.push(chars[index]);
                index += 1;
            }
            let mut height = None;
            if chars.get(index) == Some(&'h')
                && chars
                    .get(index + 1)
                    .is_some_and(|char| matches!(char, 'a'..='n' | 'p'))
            {
                height = chars.get(index + 1).copied();
                index += 2;
            }
            glyphs.push(Glyph {
                code,
                name,
                components,
                modifiers,
                height,
            });
            if chars.get(index) == Some(&'!') {
                index += 1;
                continue;
            }
            break;
        }
        let mut puncta = Vec::new();
        while index + 1 < chars.len()
            && (chars[index] == 's' || chars[index] == 'p')
            && chars[index + 1] == 'u'
        {
            let rising = chars[index] == 'p';
            index += 2;
            if chars
                .get(index)
                .is_some_and(|char| is_modifier_letter(*char))
            {
                index += 1;
            }
            let count_start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            if let Ok(count) = snippet[count_start..index].parse::<usize>() {
                if count > 0 {
                    puncta.push((rising, count));
                    continue;
                }
            }
            break;
        }
        let mut letters = Vec::new();
        while index + 1 < chars.len()
            && matches!(chars[index], 'l')
            && matches!(chars[index + 1], 's' | 't')
        {
            let letter_start = index;
            index += 2;
            while index < chars.len() && !chars[index].is_ascii_digit() {
                index += 1;
            }
            let digit_index = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            if digit_index > letter_start + 2 && digit_index < index {
                let shorthand: String = chars[letter_start..digit_index].iter().collect();
                let digit = chars[digit_index].to_digit(10).unwrap_or(0) as u8;
                letters.push((shorthand, digit));
            } else {
                break;
            }
        }
        let raw: String = chars[start..index].iter().collect();
        if index == start {
            // Suite non reconnue : un seul caractère, conservé tel quel.
            index += 1;
            let unknown: String = chars[start..index].iter().collect();
            result.push(Descriptor {
                spacing: String::new(),
                glyphs: Vec::new(),
                puncta: Vec::new(),
                letters: Vec::new(),
                raw: unknown,
            });
            continue;
        }
        result.push(Descriptor {
            spacing,
            glyphs,
            puncta,
            letters,
            raw,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cardines_commented_example() {
        let descriptors = parse("```po////pe>2lse7lsl3qlhh!vshhppt1sut2ql>ppu3");
        assert!(descriptors.len() >= 4, "{descriptors:?}");
        let names: Vec<_> = descriptors
            .iter()
            .flat_map(|descriptor| descriptor.glyphs.iter().map(|glyph| glyph.name))
            .collect();
        assert!(names.contains(&"porrectus"));
        assert!(names.contains(&"pes"));
        assert!(names.contains(&"quilisma"));
        assert!(names.contains(&"virga-strata"));
        assert_eq!(place(5), None, "Laon's 'inside' has no MEI @place");
        assert_eq!(place(2), Some("above"));
        assert_eq!(interval('f', 'g'), "u");
        assert_eq!(interval('g', 'f'), "d");
        assert_eq!(interval('f', 'f'), "s");
    }

    #[test]
    fn parses_the_versioned_nabc_examples() {
        for snippet in [
            "talsi8",
            "pe",
            "vi",
            "vihh",
            "ta>",
            "cl-lse4",
            "pfhhlsl2lsc2",
            "bv-",
            "vi-hhppu2su1sut1",
            "ql-ppt1vi-hj",
            "toS",
            "vi-tohhlsi7lss6",
        ] {
            let descriptors = parse(snippet);
            assert!(!descriptors.is_empty(), "{snippet}");
            assert_eq!(
                descriptors
                    .iter()
                    .map(|descriptor| descriptor.raw.as_str())
                    .collect::<String>(),
                snippet,
                "the exact NABC text must be recoverable from the descriptors"
            );
        }
    }
}
