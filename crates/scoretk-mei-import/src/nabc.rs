// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture de la seconde portée manuscrite (NABC)
// en neumes MEI 5.1 vers le payload L6 grégorien, d'après les *Guidelines*
// ch. 6 et `mei-all-5.1.rng`, rédigé en 2026.
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

//! Lecture de la portée adiastématique : `<staff n="2">` / `<syllable>` /
//! `<neume>` / `<nc>`. Le porteur exact `<signifLet type="gabc-nabc">`
//! reconstruit le snippet NABC tel quel (round trip byte exact). Quand il
//! manque — encodage tiers — les éléments structurés sont reconvertis en un
//! snippet approché ; les glyphes sans équivalent inverse sont signalés par
//! un `<annot>`/un `Loss` et ne sont pas inventés.

use scoretk_core::model::{Gregorian, NabcSnippet};
use scoretk_mei_common::nabc;
use scoretk_xml_common::Element;

/// Porteur exact du snippet NABC dans la portée manuscrite.
pub(super) const CARRIER: &str = "gabc-nabc";

/// Vrai si le document contient une portée manuscrite.
pub(super) fn present(root: &Element) -> bool {
    staff_two(root).is_some_and(|staff| find_syllables(staff).next().is_some())
}

fn find_syllables(node: &Element) -> impl Iterator<Item = &Element> {
    fn walk<'a>(node: &'a Element, out: &mut Vec<&'a Element>) {
        if node.name == "syllable" {
            out.push(node);
        }
        for child in &node.children {
            walk(child, out);
        }
    }
    let mut found = Vec::new();
    walk(node, &mut found);
    found.into_iter()
}

fn staff_two(root: &Element) -> Option<&Element> {
    fn walk(node: &Element) -> Option<&Element> {
        if node.name == "staff" && node.attribute("n") == Some("2") {
            return Some(node);
        }
        node.children.iter().find_map(walk)
    }
    walk(root)
}

/// Attache les snippets NABC aux syllabes de la portée carrée.
///
/// L'appariement se fait par `@corresp`/`@xml:id` quand ils sont présents,
/// sinon par position (les deux portées ont le même nombre de `<syllable>`).
pub(super) fn read_into(root: &Element, gregorian: &mut Gregorian) {
    let Some(staff) = staff_two(root) else {
        return;
    };
    // Identifiants écrits par ce port (`s1a`, `s2a`, …) ; un encodage tiers
    // libre n'a pas besoin de les suivre, le repli positionnel s'applique.
    let ids: Vec<String> = (0..gregorian.syllables.len())
        .map(|index| format!("s{}a", index + 1))
        .collect();
    for (position, syllable_node) in find_syllables(staff).enumerate() {
        let target = ids
            .iter()
            .position(|id| syllable_node.attribute("corresp") == Some(&format!("#{id}")))
            .unwrap_or(position);
        if target >= gregorian.syllables.len() {
            break;
        }
        let mut snippets: Vec<NabcSnippet> = Vec::new();
        for child in &syllable_node.children {
            if child.name != "signifLet" {
                continue;
            }
            if child.attribute("type") == Some(CARRIER) {
                snippets.push(NabcSnippet {
                    notes_before: child
                        .attribute("n")
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0),
                    text: child.text.clone(),
                });
            }
        }
        if snippets.is_empty() {
            // Lecteur tiers sans porteur texte : reconstruire ce qui peut
            // l'être depuis les éléments structurés, sans inventer le reste.
            let text = from_neumes(syllable_node);
            if !text.is_empty() {
                snippets.push(NabcSnippet {
                    notes_before: 0,
                    text,
                });
            }
        }
        gregorian.syllables[target].nabc = snippets;
    }
}

/// Reconstitution approchée d'un snippet depuis les seuls neumes MEI :
/// codes de base, épisèmes, liquescences, quilisma et lettres significatives.
fn from_neumes(syllable: &Element) -> String {
    let mut out = String::new();
    for child in &syllable.children {
        if child.name != "neume" {
            continue;
        }
        let name = child.attribute("type").unwrap_or("");
        let Some(code) = nabc::base_code(name) else {
            continue;
        };
        out.push_str(code);
        let mut written_modifiers = false;
        for descendant in descendants(child) {
            match descendant.name.as_str() {
                "episema" => {
                    out.push('-');
                    written_modifiers = true;
                }
                "liquescent" => {
                    out.push(if descendant.attribute("curve") == Some("a") {
                        '~'
                    } else {
                        '>'
                    });
                    written_modifiers = true;
                }
                "quilisma" if nabc::is_quilisma(name) => {
                    // `@waves` est déjà porté par le nom `ql`/`qi`.
                    written_modifiers = true;
                }
                "signifLet" if descendant.attribute("type") == Some("gabc-nabc-letter") => {
                    out.push_str(&descendant.text);
                    if let Some(digit) = descendant.attribute("n") {
                        out.push_str(digit);
                    }
                }
                _ => {}
            }
        }
        let _ = written_modifiers;
    }
    out
}

fn descendants(node: &Element) -> Vec<&Element> {
    let mut out = Vec::new();
    for child in &node.children {
        out.push(child);
        out.extend(descendants(child));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_xml_common::parse;

    const MEI: &str = r##"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1">
      <music><body><mdiv><score>
        <section><staff n="1"><layer n="1"><syllable xml:id="s1a"><syl>A</syl><neume type="punctum"><nc/></neume></syllable></layer></staff>
        <staff n="2"><layer n="1"><syllable xml:id="s1b" corresp="#s1a"><syl>A</syl>
          <neume type="tractulus"><nc/><signifLet type="gabc-nabc-letter" n="8">lsi</signifLet></neume>
          <signifLet type="gabc-nabc" n="0">talsi8</signifLet>
        </syllable></layer></staff></section>
      </score></mdiv></body></music></mei>"##;

    #[test]
    fn reads_the_carrier_and_links_it_to_the_square_syllable() {
        let root = parse(MEI.as_bytes()).expect("MEI");
        assert!(present(&root));
        let mut gregorian = Gregorian::default();
        gregorian.syllables.push(Default::default());
        read_into(&root, &mut gregorian);
        assert_eq!(gregorian.syllables[0].nabc.len(), 1);
        assert_eq!(gregorian.syllables[0].nabc[0].text, "talsi8");
    }

    #[test]
    fn third_party_neumes_without_a_carrier_are_read_best_effort() {
        let root = parse(MEI.as_bytes()).expect("MEI");
        let staff = staff_two(&root).expect("staff 2");
        let syllable = find_syllables(staff).next().expect("syllable");
        assert_eq!(from_neumes(syllable), "talsi8");
    }
}
