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

//! Arbre XML minimal construit avec `quick-xml`.
//!
//! Le lecteur `.mscx` a besoin de parcourir librement les enfants d'un élément
//! (l'ordre des balises n'est pas toujours celui du calcul des durées), d'où un
//! petit arbre en mémoire plutôt qu'un parcours événementiel. Les fichiers
//! `.mscx` font quelques dizaines de kilo-octets : le coût est négligeable et
//! rien n'est écrit sur disque.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::{Error, Result};
use scoretk_core::model::Fraction;

/// Élément XML : nom, attributs, texte direct et enfants.
#[derive(Clone, Debug, Default)]
pub struct Element {
    /// Nom qualifié tel qu'écrit (`Staff`, `durationType`…).
    pub name: String,
    /// Attributs, dans l'ordre du fichier.
    pub attrs: Vec<(String, String)>,
    /// Texte direct de l'élément (feuilles uniquement en pratique).
    pub text: String,
    /// Contenu mixte re-sérialisé, balises internes comprises.
    ///
    /// Sert aux textes riches MuseScore, dont le balisage (`<b>`, `<font
    /// face="…">`, `<sym>`) est conservé tel quel : `Element::text` perd l'ordre
    /// du texte et des enfants, `raw_inner` ne le perd pas. Correspond à ce que
    /// `XmlReader::readXml` (`engraving/rw/xmlreader.cpp`) renvoie au lecteur
    /// C++.
    pub raw_inner: String,
    /// Enfants, dans l'ordre du fichier.
    pub children: Vec<Element>,
}

/// Retire le balisage d'un contenu mixte, en gardant le texte des enfants.
fn strip_markup(raw: &str) -> String {
    let mut plain = String::new();
    let mut rest = raw;
    while let Some(position) = rest.find('<') {
        plain.push_str(&rest[..position]);
        let Some(end) = rest[position..].find('>') else {
            break;
        };
        let tag = rest[position + 1..position + end]
            .trim()
            .trim_end_matches('/');
        if tag == "br" {
            plain.push('\n');
        }
        rest = &rest[position + end + 1..];
    }
    plain.push_str(rest);
    plain
}

impl Element {
    /// Valeur d'un attribut.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Attribut entier.
    pub fn attr_i64(&self, name: &str) -> Option<i64> {
        self.attr(name).and_then(|value| value.trim().parse().ok())
    }

    /// Premier enfant portant ce nom.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Tous les enfants portant ce nom, dans l'ordre.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }

    /// Vrai si un enfant porte ce nom.
    pub fn has_child(&self, name: &str) -> bool {
        self.child(name).is_some()
    }

    /// Texte direct, sans espaces de bord.
    pub fn text(&self) -> &str {
        self.text.trim()
    }

    /// Nom d'instrument hérité : `<br/>` devient un saut de ligne.
    ///
    /// Portage de `TRead::readLegacyStaffName` (`read500/tread.cpp:4200`) et
    /// `lineBreakFromTag` (`:3812`). L'export `toPlainText`
    /// (`musicxmlfonthandler.cpp:56`) retire le balisage restant ; le modèle
    /// ne garde que du texte, on produit donc directement ce texte, avec les
    /// sauts de ligne. La norme MusicXML est muette sur cette lecture : on
    /// suit le comportement de MuseScore 4.7.4.
    /// Texte de l'élément sans balisage inline (`<i>`, `<b>`, `<font>`…).
    ///
    /// `TRead::read(Lyrics*)` (`read500/tread.cpp`) lit `<text>` avec
    /// `XmlReader::readXml()`, qui conserve le texte imbriqué. Notre modèle
    /// garde du texte plat : on retire les balises, `<br/>` devenant un saut
    /// de ligne comme `lineBreakFromTag` (`tread.cpp:3812`).
    pub fn plain_text(&self) -> String {
        strip_markup(&self.raw_inner)
    }

    pub fn legacy_staff_name(&self) -> String {
        unescape(strip_markup(&self.raw_inner).trim())
    }

    /// Texte direct converti en entier.
    pub fn int(&self) -> Option<i64> {
        self.text().parse().ok()
    }

    /// Texte direct converti en fraction (`"3/4"` ou `"480"`).
    pub fn fraction(&self) -> Option<Fraction> {
        parse_fraction(self.text())
    }

    /// Attribut converti en fraction.
    pub fn attr_fraction(&self, name: &str) -> Option<Fraction> {
        self.attr(name).and_then(parse_fraction)
    }
}

/// Analyse un littéral de fraction MuseScore.
///
/// `XmlReader::readFraction` (`engraving/rw/xmlreader.cpp`) accepte `"z/n"` et
/// un entier seul ; les anciens attributs `z`/`n` sont l'équivalent de `z/n`.
pub fn parse_fraction(text: &str) -> Option<Fraction> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some((numerator, denominator)) = text.split_once('/') {
        let numerator: i64 = numerator.trim().parse().ok()?;
        let denominator: i64 = denominator.trim().parse().ok()?;
        Fraction::try_new(numerator, denominator)
    } else {
        text.parse::<i64>().ok().map(Fraction::from_integer)
    }
}

/// Construit l'arbre à partir des octets d'un `.mscx`.
pub fn parse(bytes: &[u8]) -> Result<Element> {
    let mut reader = Reader::from_reader(bytes);
    // Le texte n'est pas rogné globalement : les textes riches MuseScore
    // contiennent des espaces significatifs (`<font face="Edwin"></font> = 80`).
    // Les accesseurs rognent eux-mêmes (`Element::text`, `int`, `fraction`).
    reader.config_mut().trim_text(false);

    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                let element = element_from_start(&start)?;
                stack.push(element);
            }
            Event::Empty(start) => {
                let element = element_from_start(&start)?;
                if let Some(parent) = stack.last_mut() {
                    parent.raw_inner.push_str(&start_tag(&element, true));
                }
                attach(&mut stack, &mut root, element);
            }
            Event::End(_) => {
                if let Some(element) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.raw_inner.push_str(&start_tag(&element, false));
                        parent.raw_inner.push_str(&element.raw_inner);
                        parent.raw_inner.push_str("</");
                        parent.raw_inner.push_str(&element.name);
                        parent.raw_inner.push('>');
                    }
                    attach(&mut stack, &mut root, element);
                }
            }
            Event::Text(text) => {
                let raw = text.xml_content(XmlVersion::Implicit1_0);
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&unescape(&raw));
                    escape_into(&mut current.raw_inner, &raw);
                }
            }
            Event::GeneralRef(reference) => {
                let entity = reference.as_ref();
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&decode_reference(entity));
                    current.raw_inner.push('&');
                    current.raw_inner.push_str(entity);
                    current.raw_inner.push(';');
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    root.ok_or(Error::MissingScore)
}

fn element_from_start(start: &BytesStart<'_>) -> Result<Element> {
    let mut attrs = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute?;
        let value = attribute.normalized_value(XmlVersion::Implicit1_0)?;
        attrs.push((attribute.key.as_ref().to_owned(), value.into_owned()));
    }
    Ok(Element {
        name: start.name().as_ref().to_owned(),
        attrs,
        text: String::new(),
        raw_inner: String::new(),
        children: Vec::new(),
    })
}

/// Re-sérialise une balise ouvrante, pour `Element::raw_inner`.
fn start_tag(element: &Element, self_closing: bool) -> String {
    let mut out = String::new();
    out.push('<');
    out.push_str(&element.name);
    for (name, value) in &element.attrs {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape_into(&mut out, value);
        out.push('"');
    }
    out.push_str(if self_closing { "/>" } else { ">" });
    out
}

/// Échappe `<`, `>` et `&` pour reconstruire un balisage interne.
fn escape_into(buffer: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => buffer.push_str("&amp;"),
            '<' => buffer.push_str("&lt;"),
            '>' => buffer.push_str("&gt;"),
            other => buffer.push(other),
        }
    }
}

/// Décode les entités XML prédéfinies et les références numériques.
///
/// Les entités nommées non standard (`&ucirc;`, fréquentes dans les anciens
/// fichiers MuseScore) sont laissées telles quelles : les supprimer
/// silencieusement perdrait du texte.
fn unescape(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find('&') {
        output.push_str(&rest[..position]);
        let tail = &rest[position..];
        let Some(end) = tail.find(';') else {
            output.push('&');
            rest = &tail[1..];
            continue;
        };
        output.push_str(&decode_reference(&tail[1..end]));
        rest = &tail[end + 1..];
    }
    output.push_str(rest);
    output
}

/// Décode une référence d'entité (`amp`, `#39`, `#x2019`).
///
/// Une entité nommée inconnue est rendue telle quelle (`&ucirc;`), pour ne pas
/// perdre le texte des anciens fichiers MuseScore.
fn decode_reference(entity: &str) -> String {
    let decoded = match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| {
                entity
                    .strip_prefix('#')
                    .and_then(|decimal| decimal.parse().ok())
            })
            .and_then(char::from_u32),
    };
    match decoded {
        Some(character) => character.to_string(),
        None => format!("&{entity};"),
    }
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(element),
        None => {
            if root.is_none() {
                *root = Some(element);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbre_simple() {
        let xml = br#"<museScore version="4.60"><Score><Division>480</Division>
            <Part id="1"><trackName>Fl&ucirc;te</trackName></Part></Score></museScore>"#;
        let root = parse(xml).unwrap();
        assert_eq!(root.name, "museScore");
        assert_eq!(root.attr("version"), Some("4.60"));
        let score = root.child("Score").unwrap();
        assert_eq!(score.child("Division").unwrap().int(), Some(480));
        let part = score.child("Part").unwrap();
        assert_eq!(part.attr_i64("id"), Some(1));
        assert_eq!(part.child("trackName").unwrap().text(), "Fl&ucirc;te");
    }

    #[test]
    fn fractions() {
        assert_eq!(parse_fraction("3/4"), Some(Fraction::new(3, 4)));
        assert_eq!(parse_fraction("480"), Some(Fraction::from_integer(480)));
        assert_eq!(parse_fraction("4/0"), None);
        assert_eq!(parse_fraction(""), None);
        assert_eq!(parse_fraction("x/y"), None);
    }

    #[test]
    fn contenu_mixte_des_textes() {
        // Le balisage des textes riches doit survivre tel quel, ordre compris :
        // `Element::text` ne garde que le texte direct, `raw_inner` tout.
        let xml = br#"<museScore version="4.60"><Score><Staff id="1"><Measure><voice>
            <Tempo><tempo>1.333333</tempo>
            <text><sym>metNoteQuarterUp</sym><font face="Edwin"></font> = 80</text>
            </Tempo></voice></Measure></Staff></Score></museScore>"#;
        let root = parse(xml).unwrap();
        let score = root.child("Score").unwrap();
        let staff = score.child("Staff").unwrap();
        let measure = staff.child("Measure").unwrap();
        let voice = measure.child("voice").unwrap();
        let tempo = voice.child("Tempo").unwrap();
        let text = tempo.child("text").unwrap();
        assert_eq!(
            text.raw_inner,
            "<sym>metNoteQuarterUp</sym><font face=\"Edwin\"></font> = 80"
        );
        // Le texte direct est conservé sans rognage dans `text` ; `text()` rogne.
        assert_eq!(text.text, " = 80");
        assert_eq!(text.text(), "= 80");
        assert_eq!(tempo.child("tempo").unwrap().text(), "1.333333");
    }
}
