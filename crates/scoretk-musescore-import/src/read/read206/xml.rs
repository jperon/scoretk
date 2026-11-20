// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/xmlreader.cpp` de MuseScore Studio (lecture des
// attributs et du contenu mixte des textes), Copyright (C) MuseScore Limited
// and others, modifié (réécrit en Rust) en 2026.
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

//! Arbre XML minimal, en octets propres, pour l'adaptation des fichiers 2.x.
//!
//! `Read206::readScoreFile` reçoit un `XmlReader` flux ; le portage 3.x lit,
//! lui, un arbre déjà assemblé (`read302::xml`). Pour réutiliser ce dernier
//! sans le modifier, on analyse ici le `.mscx` 2.x en un arbre équivalent dont
//! les nœuds sont possédés, on réécrit sa structure (voix, n-olets) puis on le
//! sérialise vers le lecteur 3.x. Le contenu mixte des `<text>` est conservé
//! dans l'ordre du document, comme `XmlReader::readXml`.

use std::borrow::Cow;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::{Error, Result};

/// Contenu d'un élément, dans l'ordre du document.
#[derive(Clone, Debug)]
pub(super) enum Content {
    Text(String),
    Element(Node),
}

/// Élément XML possédé (nom, attributs, contenu ordonné).
#[derive(Clone, Debug)]
pub(super) struct Node {
    pub(super) name: String,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) content: Vec<Content>,
}

impl Node {
    pub(super) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            content: Vec::new(),
        }
    }

    /// Valeur d'un attribut.
    pub(super) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub(super) fn attr_i32(&self, name: &str) -> Option<i32> {
        self.attr(name)?.trim().parse().ok()
    }

    pub(super) fn children(&self) -> impl Iterator<Item = &Node> {
        self.content.iter().filter_map(|item| match item {
            Content::Element(node) => Some(node),
            Content::Text(_) => None,
        })
    }

    pub(super) fn children_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.content.iter_mut().filter_map(|item| match item {
            Content::Element(node) => Some(node),
            Content::Text(_) => None,
        })
    }

    pub(super) fn child(&self, name: &str) -> Option<&Node> {
        self.children().find(|child| child.name == name)
    }

    pub(super) fn push(&mut self, child: Node) {
        self.content.push(Content::Element(child));
    }

    pub(super) fn child_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.children_mut().find(|child| child.name == name)
    }

    /// Retire tous les enfants portant ce nom.
    pub(super) fn retain_without(&mut self, name: &str) {
        self.content
            .retain(|item| !matches!(item, Content::Element(node) if node.name == name));
    }

    /// Texte de l'élément et de ses descendants, dans l'ordre du document.
    pub(super) fn text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        for item in &self.content {
            match item {
                Content::Text(text) => out.push_str(text),
                Content::Element(node) => node.collect_text(out),
            }
        }
    }

    pub(super) fn trimmed_text(&self) -> String {
        self.text().trim().to_owned()
    }

    pub(super) fn int_text(&self) -> Option<i32> {
        self.trimmed_text().parse().ok()
    }

    /// Sérialise l'élément, en échappant le texte comme `toXmlEscaped`.
    pub(super) fn write_xml(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (name, value) in &self.attrs {
            out.push(' ');
            out.push_str(name);
            out.push_str("=\"");
            escape(out, value);
            out.push('"');
        }
        out.push('>');
        for item in &self.content {
            match item {
                Content::Text(text) => escape(out, text),
                Content::Element(node) => node.write_xml(out),
            }
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push_str(">\n");
    }
}

fn escape(out: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
}

/// Analyse un `.mscx` en arbre possédé.
pub(super) fn parse(text: &str) -> Result<Node> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;

    loop {
        match reader.read_event()? {
            Event::Start(start) => stack.push(element(&start)),
            Event::Empty(start) => {
                let node = element(&start);
                attach(&mut stack, &mut root, node);
            }
            Event::End(_) => {
                if let Some(node) = stack.pop() {
                    attach(&mut stack, &mut root, node);
                }
            }
            Event::Text(chunk) => push_text(&mut stack, chunk.into_inner().as_ref()),
            Event::CData(chunk) => push_text(&mut stack, chunk.into_inner().as_ref()),
            Event::GeneralRef(reference) => {
                if let Some(resolved) = resolve_reference(reference.into_inner().as_ref()) {
                    push_text(&mut stack, &resolved);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| {
        Error::Xml(quick_xml::errors::IllFormedError::MissingEndTag("museScore".to_owned()).into())
    })
}

fn element(start: &BytesStart<'_>) -> Node {
    let name = start.name().into_inner().to_owned();
    let mut attrs = Vec::new();
    for attribute in start.attributes().flatten() {
        let key = attribute.key.into_inner().to_owned();
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .unwrap_or(Cow::Borrowed(""))
            .into_owned();
        attrs.push((key, value));
    }
    Node {
        name,
        attrs,
        content: Vec::new(),
    }
}

fn attach(stack: &mut [Node], root: &mut Option<Node>, node: Node) {
    match stack.last_mut() {
        Some(parent) => parent.content.push(Content::Element(node)),
        None => {
            if root.is_none() {
                *root = Some(node);
            }
        }
    }
}

fn push_text(stack: &mut [Node], text: &str) {
    if let Some(node) = stack.last_mut() {
        match node.content.last_mut() {
            Some(Content::Text(existing)) => existing.push_str(text),
            _ => node.content.push(Content::Text(text.to_owned())),
        }
    }
}

fn resolve_reference(name: &str) -> Option<String> {
    let name = name.trim_end_matches(';');
    if let Some(number) = name.strip_prefix('#') {
        let code = match number
            .strip_prefix('x')
            .or_else(|| number.strip_prefix('X'))
        {
            Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code).map(|character| character.to_string());
    }
    let character = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => return Some(format!("&{name};")),
    };
    Some(character.to_owned())
}
