// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : il remplace le `XmlStreamReader` de MuseScore
// Studio par un arbre XML en mémoire, sans logique de conversion.
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

//! Arbre XML minimal pour la lecture MusicXML.
//!
//! L'import de MuseScore travaille en deux passes sur le même document
//! (`importmusicxmlpass1.cpp` puis `importmusicxmlpass2.cpp`) : plutôt qu'un
//! lecteur en flux avec reprise, ce module matérialise l'arbre. Les documents
//! MusicXML restent petits (quelques milliers de lignes) et tout tient en
//! mémoire, ce qui est compatible avec la cible wasm.

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

use crate::error::{Error, Result};

/// Élément XML : nom local (préfixe de namespace retiré), attributs, texte
/// direct et enfants.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// Nom local de l'élément, sans préfixe de namespace.
    pub name: String,
    /// Attributs dans l'ordre du document, valeurs dé-échappées.
    pub attributes: Vec<(String, String)>,
    /// Texte direct (concaténation des nœuds texte et CDATA, non découpé).
    pub text: String,
    /// Enfants, dans l'ordre du document.
    pub children: Vec<Element>,
}

impl Element {
    /// Valeur d'un attribut.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Texte de l'élément, sans espaces de bord.
    pub fn text_trimmed(&self) -> &str {
        self.text.trim()
    }

    /// Premier enfant portant ce nom.
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Enfants portant ce nom.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }

    /// Texte du premier enfant portant ce nom.
    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(Element::text_trimmed)
    }

    /// Entier du premier enfant portant ce nom.
    pub fn child_int(&self, name: &str) -> Option<i64> {
        self.child_text(name)?.parse().ok()
    }

    /// Nombre flottant du premier enfant portant ce nom.
    pub fn child_f64(&self, name: &str) -> Option<f64> {
        self.child_text(name)?.replace(',', ".").parse().ok()
    }

    /// Vrai si l'élément enfant existe (quel que soit son contenu).
    pub fn has_child(&self, name: &str) -> bool {
        self.child(name).is_some()
    }
}

/// Analyse un document XML et renvoie sa racine.
pub fn parse(bytes: &[u8]) -> Result<Element> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = false;

    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => stack.push(start_element(&event)?),
            Ok(Event::Empty(event)) => {
                let element = start_element(&event)?;
                attach(&mut stack, &mut root, element);
            }
            Ok(Event::End(_)) => {
                if let Some(element) = stack.pop() {
                    attach(&mut stack, &mut root, element);
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&event.xml10_content());
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(event.as_ref());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(Error::Xml(error)),
        }
    }

    root.ok_or(Error::UnknownRoot(String::new()))
}

fn attach(stack: &mut Vec<Element>, root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(element),
        None => {
            if root.is_none() {
                *root = Some(element);
            }
        }
    }
}

fn start_element(event: &quick_xml::events::BytesStart<'_>) -> Result<Element> {
    let name = local_name(event.name().as_ref());
    let mut attributes = Vec::new();
    for attribute in event.attributes() {
        let attribute = attribute?;
        let key = local_name(attribute.key.as_ref());
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map(|value| value.into_owned())
            .unwrap_or_default();
        attributes.push((key, value));
    }
    Ok(Element {
        name,
        attributes,
        text: String::new(),
        children: Vec::new(),
    })
}

/// Nom local d'un nom qualifié (`ns:element` → `element`, `ns:attr` → `attr`).
fn local_name(name: &str) -> String {
    match name.rsplit_once(':') {
        Some((_, local)) => local.to_owned(),
        None => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbre_simple() {
        let root = parse(
            br#"<score-partwise version="4.0">
                  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
                  <part id="P1"><measure number="1"><note><pitch><step>C</step><octave>4</octave></pitch></note></measure></part>
                </score-partwise>"#,
        )
        .unwrap();
        assert_eq!(root.name, "score-partwise");
        assert_eq!(root.attribute("version"), Some("4.0"));
        let part = root.child("part").unwrap();
        assert_eq!(part.attribute("id"), Some("P1"));
        let note = part.child("measure").unwrap().child("note").unwrap();
        assert_eq!(note.child_text("pitch").is_none(), true);
        assert_eq!(note.child("pitch").unwrap().child_text("step"), Some("C"));
    }

    #[test]
    fn namespace_retire() {
        let root = parse(br#"<x:score-partwise xmlns:x="urn:x"><x:part/></x:score-partwise>"#)
            .unwrap();
        assert_eq!(root.name, "score-partwise");
        assert_eq!(root.children[0].name, "part");
    }
}
