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

//! Mini DOM en lecture seule pour les fichiers `.mscx`.
//!
//! `quick_xml` expose un flux d'événements ; ce module l'assemble en un arbre
//! simple avant la lecture. Un arbre évite d'avoir à rejouer le flux pour les
//! éléments dont l'ordre des enfants diffère de ce qu'attend le C++ (les
//! `<dots>` d'un accord peuvent suivre son `<durationType>`, alors que le
//! lecteur C++ les exige avant) et permet de lire un `<Part>` après avoir
//! rencontré son `<Staff>`.
//!
//! Tout reste en mémoire : aucune entrée/sortie, aucun thread.

use std::borrow::Cow;

use quick_xml::errors::IllFormedError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::{Error, Result};
use scoretk_core::model::Fraction;

/// Divisions MuseScore par ronde (`Constants::DIVISION` = 480 par noire).
///
/// Sert à convertir les valeurs écrites en ticks bruts (`<tick>`, `<ticks>`)
/// par `Fraction::fromTicks` dans MuseScore.
pub(crate) const TICKS_PER_WHOLE: u32 = 4 * scoretk_core::model::DEFAULT_DIVISION;

/// Contenu d'un élément, dans l'ordre du document.
///
/// Les tranches empruntent le texte analysé (`parse`) ; seules les valeurs
/// normalisées ou les entités résolues sont allouées.
enum Content<'a> {
    Text(Cow<'a, str>),
    Element(Node<'a>),
}

/// Élément XML.
pub(crate) struct Node<'a> {
    name: Cow<'a, str>,
    attrs: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    content: Vec<Content<'a>>,
}

impl<'a> Node<'a> {
    /// Nom de l'élément.
    pub(crate) fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Valeur d'un attribut.
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
    }

    /// Attribut entier.
    pub(crate) fn attr_i32(&self, name: &str) -> Option<i32> {
        self.attr(name)?.trim().parse().ok()
    }

    /// Premier enfant portant ce nom.
    pub(crate) fn child(&self, name: &str) -> Option<&Node<'a>> {
        self.children().find(|child| child.name == name)
    }

    /// Enfants éléments, dans l'ordre du document.
    pub(crate) fn children(&self) -> impl Iterator<Item = &Node<'a>> {
        self.content.iter().filter_map(|item| match item {
            Content::Element(node) => Some(node),
            Content::Text(_) => None,
        })
    }

    /// Enfants éléments portant ce nom.
    pub(crate) fn children_named<'s>(
        &'s self,
        name: &'s str,
    ) -> impl Iterator<Item = &'s Node<'a>> + 's {
        self.children().filter(move |child| child.name == name)
    }

    /// Texte de l'élément et de ses descendants, dans l'ordre du document.
    ///
    /// Reproduit `Text::readXmlText` : un `<text>` peut mêler du texte et des
    /// `<font>`, dont le contenu textuel compte aussi.
    pub(crate) fn text(&self) -> String {
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

    /// Contenu mixte re-sérialisé en balisage, pour les textes riches.
    ///
    /// `Node` conserve l'ordre du texte et des éléments : on peut donc
    /// reconstituer les balises internes d'un `<text>` (`<b>`, `<font
    /// face="…">`, `<sym>`…), comme `XmlReader::readXml` le fait pour le
    /// lecteur C++. Les caractères réservés sont ré-échappés, guillemet
    /// compris, comme `String::toXmlEscaped` dans MuseScore : le texte interne
    /// contient donc `&quot;` et l'export l'écrit tel quel.
    pub(crate) fn inner_markup(&self) -> String {
        let mut out = String::new();
        self.write_content(&mut out);
        out
    }

    /// Contenu mixte re-sérialisé **sans échappement**, éléments vides
    /// auto-fermants compris.
    ///
    /// Les noms d'instruments de MuseScore 3.x gardent leur balisage
    /// (`<font size="…"></font>Piano`) et `Instrument::nameAsPlainText` le rend
    /// tel quel, sans ré-échapper ; `writeDisplayName` échappe ensuite une
    /// seule fois le tout.
    pub(crate) fn inner_markup_raw_self_closing(&self) -> String {
        let mut out = String::new();
        self.write_content_raw_self_closing(&mut out);
        out
    }

    fn write_content_raw_self_closing(&self, out: &mut String) {
        for item in &self.content {
            match item {
                Content::Text(text) => out.push_str(text),
                Content::Element(node) => node.write_markup_raw_self_closing(out),
            }
        }
    }

    fn write_markup_raw_self_closing(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (name, value) in &self.attrs {
            out.push(' ');
            out.push_str(name);
            out.push_str("=\"");
            out.push_str(value);
            out.push('"');
        }
        if self.content.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        self.write_content_raw_self_closing(out);
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }

    fn write_content(&self, out: &mut String) {
        for item in &self.content {
            match item {
                Content::Text(text) => escape_markup(out, text),
                Content::Element(node) => node.write_markup(out),
            }
        }
    }

    fn write_markup(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (name, value) in &self.attrs {
            out.push(' ');
            out.push_str(name);
            out.push_str("=\"");
            escape_markup(out, value);
            out.push('"');
        }
        out.push('>');
        self.write_content(out);
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }

    /// Texte débarrassé des espaces de début et de fin.
    pub(crate) fn trimmed_text(&self) -> String {
        self.text().trim().to_owned()
    }

    /// Texte entier.
    pub(crate) fn int_text(&self) -> Option<i32> {
        self.trimmed_text().parse().ok()
    }

    /// Fraction, lue comme `XmlReader::readFraction`.
    ///
    /// Accepte `"3/4"`, un entier interprété comme des ticks MuseScore
    /// (`1920` = une ronde) et les anciens attributs `z`/`n`.
    pub(crate) fn fraction(&self) -> Option<Fraction> {
        let text = self.trimmed_text();
        if let Some(fraction) = parse_fraction(&text) {
            return Some(fraction);
        }
        match (self.attr_i32("z"), self.attr_i32("n")) {
            (Some(z), Some(n)) => Fraction::try_new(i64::from(z), i64::from(n)),
            (Some(z), None) => Fraction::try_new(i64::from(z), 1),
            _ => None,
        }
    }
}

/// Échappe le contenu textuel d'un balisage (`toXmlEscaped`).
fn escape_markup(out: &mut String, text: &str) {
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

/// Analyse une fraction écrite (`"3/4"`) ou un nombre de ticks (`"1920"`).
pub(crate) fn parse_fraction(text: &str) -> Option<Fraction> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some((numerator, denominator)) = text.split_once('/') {
        let numerator: i64 = numerator.trim().parse().ok()?;
        let denominator: i64 = denominator.trim().parse().ok()?;
        return Fraction::try_new(numerator, denominator);
    }
    let ticks: i64 = text.parse().ok()?;
    Some(Fraction::from_ticks(ticks, TICKS_PER_WHOLE))
}

/// Analyse un `.mscx` et renvoie son élément racine.
///
/// L'arbre emprunte le texte analysé : l'appelant doit le garder vivant
/// pendant toute la lecture.
pub(crate) fn parse<'a>(text: &'a str) -> Result<Node<'a>> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Node<'a>> = Vec::new();
    let mut root: Option<Node<'a>> = None;

    loop {
        match reader.read_event()? {
            Event::Start(start) => stack.push(element(text, &start)?),
            Event::Empty(start) => {
                let node = element(text, &start)?;
                attach(&mut stack, &mut root, node);
            }
            Event::End(_) => {
                if let Some(node) = stack.pop() {
                    attach(&mut stack, &mut root, node);
                }
            }
            Event::Text(chunk) => push_text(&mut stack, chunk.into_inner()),
            Event::CData(chunk) => push_text(&mut stack, chunk.into_inner()),
            Event::GeneralRef(reference) => {
                if let Some(resolved) = resolve_reference(&reference.into_inner()) {
                    push_text(&mut stack, resolved);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    // Un document sans élément racine n'est pas du XML exploitable. On réutilise
    // l'erreur « balise de fin manquante » de `quick_xml` plutôt que d'ajouter
    // une variante à `Error` (fichier partagé avec les autres lecteurs).
    root.ok_or_else(|| Error::Xml(IllFormedError::MissingEndTag("museScore".to_owned()).into()))
}

fn element<'a>(text: &'a str, start: &BytesStart<'_>) -> Result<Node<'a>> {
    let name = borrow_from(text, start.name().into_inner());
    let mut attrs = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute?;
        let key = borrow_from(text, attribute.key.into_inner());
        let value = match attribute.normalized_value(XmlVersion::Implicit1_0)? {
            Cow::Borrowed(value) => borrow_from(text, value),
            Cow::Owned(value) => Cow::Owned(value),
        };
        attrs.push((key, value));
    }
    Ok(Node {
        name,
        attrs,
        content: Vec::new(),
    })
}

/// Rend à une tranche issue du texte analysé la durée de vie de ce texte.
///
/// Les accesseurs de `quick-xml` (`BytesStart::name`, `Attributes`) empruntent
/// l'événement (`&self`) : la tranche renvoyée est bien une sous-chaîne de
/// l'entrée, mais le compilateur ne le sait pas. On retrouve donc sa position
/// par son adresse pour l'emprunter à son tour, au lieu de la recopier ; à
/// défaut (valeur normalisée ou allouée), on recopie.
fn borrow_from<'a>(text: &'a str, slice: &str) -> Cow<'a, str> {
    let offset = (slice.as_ptr() as usize).wrapping_sub(text.as_ptr() as usize);
    match text.get(offset..).filter(|tail| tail.starts_with(slice)) {
        Some(tail) => Cow::Borrowed(&tail[..slice.len()]),
        None => Cow::Owned(slice.to_owned()),
    }
}

fn attach<'a>(stack: &mut [Node<'a>], root: &mut Option<Node<'a>>, node: Node<'a>) {
    match stack.last_mut() {
        Some(parent) => parent.content.push(Content::Element(node)),
        None => {
            if root.is_none() {
                *root = Some(node);
            }
        }
    }
}

fn push_text<'a>(stack: &mut [Node<'a>], text: Cow<'a, str>) {
    if let Some(node) = stack.last_mut() {
        node.content.push(Content::Text(text));
    }
}

/// Résout une référence d'entité (`&amp;`, `&#38;`, `&#x26;`).
///
/// Les entités prédéfinies empruntent des littéraux ; les références
/// numériques et les entités inconnues (rendues telles quelles) sont allouées.
fn resolve_reference(name: &str) -> Option<Cow<'static, str>> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number
            .strip_prefix('x')
            .or_else(|| number.strip_prefix('X'))
        {
            Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code).map(|character| Cow::Owned(character.to_string()));
    }
    let character = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => return Some(Cow::Owned(format!("&{name};"))),
    };
    Some(Cow::Borrowed(character))
}
