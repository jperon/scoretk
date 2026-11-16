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

use quick_xml::errors::IllFormedError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::model::Fraction;
use crate::{Error, Result};

/// Divisions MuseScore par ronde (`Constants::DIVISION` = 480 par noire).
///
/// Sert à convertir les valeurs écrites en ticks bruts (`<tick>`, `<ticks>`)
/// par `Fraction::fromTicks` dans MuseScore.
pub(crate) const TICKS_PER_WHOLE: u32 = 4 * crate::model::DEFAULT_DIVISION;

/// Contenu d'un élément, dans l'ordre du document.
enum Content {
    Text(String),
    Element(Node),
}

/// Élément XML.
pub(crate) struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    content: Vec<Content>,
}

impl Node {
    /// Nom de l'élément.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Valeur d'un attribut.
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Attribut entier.
    pub(crate) fn attr_i32(&self, name: &str) -> Option<i32> {
        self.attr(name)?.trim().parse().ok()
    }

    /// Premier enfant portant ce nom.
    pub(crate) fn child(&self, name: &str) -> Option<&Node> {
        self.children().find(|child| child.name == name)
    }

    /// Enfants éléments, dans l'ordre du document.
    pub(crate) fn children(&self) -> impl Iterator<Item = &Node> {
        self.content.iter().filter_map(|item| match item {
            Content::Element(node) => Some(node),
            Content::Text(_) => None,
        })
    }

    /// Enfants éléments portant ce nom.
    pub(crate) fn children_named<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Node> + 'a {
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
pub(crate) fn parse(bytes: &[u8]) -> Result<Node> {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;

    loop {
        match reader.read_event()? {
            Event::Start(start) => stack.push(element(&start)?),
            Event::Empty(start) => {
                let node = element(&start)?;
                attach(&mut stack, &mut root, node);
            }
            Event::End(_) => {
                if let Some(node) = stack.pop() {
                    attach(&mut stack, &mut root, node);
                }
            }
            Event::Text(chunk) => {
                let chunk = chunk.into_inner();
                push_text(&mut stack, chunk.as_ref());
            }
            Event::CData(chunk) => {
                let chunk = chunk.into_inner();
                push_text(&mut stack, chunk.as_ref());
            }
            Event::GeneralRef(reference) => {
                let reference = reference.into_inner();
                if let Some(resolved) = resolve_reference(reference.as_ref()) {
                    push_text(&mut stack, &resolved);
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

fn element(start: &BytesStart<'_>) -> Result<Node> {
    let name = start.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute?;
        let key = attribute.key.as_ref().to_owned();
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Node {
        name,
        attrs,
        content: Vec::new(),
    })
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
        node.content.push(Content::Text(text.to_owned()));
    }
}

/// Résout une référence d'entité (`&amp;`, `&#38;`, `&#x26;`).
fn resolve_reference(name: &str) -> Option<String> {
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
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => return Some(format!("&{name};")),
    };
    Some(character.to_string())
}
