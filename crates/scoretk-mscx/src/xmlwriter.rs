// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/xmlwriter.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Écrivain XML minimal, équivalent de `XmlWriter` (`engraving/rw/xmlwriter.cpp`)
//! et de son socle `muse::XmlStreamWriter`.
//!
//! La sortie suit l'indentation de MuseScore (deux espaces, un élément par
//! ligne) et n'écrit que ce que l'appelant demande : aucune valeur n'est
//! inventée. Les textes sont échappés ; [`XmlWriter::tag_raw`] laisse passer un
//! balisage déjà construit, ce qui sert aux textes riches de MuseScore
//! (`<text><b>…</b></text>`).

/// Élément ouvert, avec de quoi le refermer sur lui-même s'il reste vide.
#[derive(Debug)]
struct Open {
    /// Position du début de l'en-tête dans la sortie.
    start: usize,
    /// En-tête rendu, indentation comprise, sans `>` final.
    head: String,
    /// Nom de l'élément.
    name: String,
    /// Vrai si un enfant ou du texte a été écrit.
    dirty: bool,
}

/// Écrivain XML en mémoire.
#[derive(Debug, Default)]
pub struct XmlWriter {
    out: String,
    stack: Vec<Open>,
    depth: usize,
}

impl XmlWriter {
    /// Construit un écrivain vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Écrit la déclaration XML (`XmlStreamWriter::startDocument`).
    pub fn start_document(&mut self) {
        self.out
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }

    /// Ouvre un élément (`XmlWriter::startElement`).
    pub fn start_element(&mut self, name: &str, attrs: &[(&str, &str)]) {
        let head = self.render_head(name, attrs);
        let start = self.out.len();
        self.out.push_str(&head);
        self.out.push_str(">\n");
        self.mark_child();
        self.stack.push(Open {
            start,
            head,
            name: name.to_owned(),
            dirty: false,
        });
        self.depth += 1;
    }

    /// Ferme l'élément courant (`XmlWriter::endElement`).
    ///
    /// Un élément resté vide est refermé sur lui-même, comme `XmlStreamWriter`.
    pub fn end_element(&mut self) {
        let Some(open) = self.stack.pop() else {
            return;
        };
        self.depth = self.depth.saturating_sub(1);
        if open.dirty {
            self.indent();
            self.out.push_str("</");
            self.out.push_str(&open.name);
            self.out.push_str(">\n");
        } else {
            self.out.truncate(open.start);
            self.out.push_str(&open.head);
            self.out.push_str("/>\n");
        }
        self.mark_child();
    }

    /// Élément purement textuel (`XmlWriter::tag`).
    pub fn tag(&mut self, name: &str, text: &str) {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        self.out.push('>');
        escape_into(&mut self.out, text);
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
        self.mark_child();
    }

    /// Élément textuel avec attributs (`XmlWriter::tag(name, attrs, body)`).
    pub fn tag_with(&mut self, name: &str, attrs: &[(&str, &str)], text: &str) {
        self.indent();
        self.out.push_str(&self.render_head(name, attrs));
        self.out.push('>');
        escape_into(&mut self.out, text);
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
        self.mark_child();
    }

    /// Élément textuel dont le contenu est un balisage déjà écrit.
    pub fn tag_raw(&mut self, name: &str, raw: &str) {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        self.out.push('>');
        self.out.push_str(raw);
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
        self.mark_child();
    }

    /// Élément vide avec attributs (`XmlWriter::tag(name, attrs)`).
    pub fn tag_empty(&mut self, name: &str, attrs: &[(&str, &str)]) {
        self.indent();
        self.out.push_str(&self.render_head(name, attrs));
        self.out.push_str("/>\n");
        self.mark_child();
    }

    /// Élément vide sans attribut.
    pub fn tag_empty_plain(&mut self, name: &str) {
        self.tag_empty(name, &[]);
    }

    /// Termine le document et renvoie son texte.
    pub fn finish(self) -> String {
        self.out
    }

    fn render_head(&self, name: &str, attrs: &[(&str, &str)]) -> String {
        let mut head = String::new();
        head.push_str(&" ".repeat(self.depth * 2));
        head.push('<');
        head.push_str(name);
        for (key, value) in attrs {
            head.push(' ');
            head.push_str(key);
            head.push_str("=\"");
            escape_into(&mut head, value);
            head.push('"');
        }
        head
    }

    fn indent(&mut self) {
        self.out.push_str(&" ".repeat(self.depth * 2));
    }

    fn mark_child(&mut self) {
        if let Some(open) = self.stack.last_mut() {
            open.dirty = true;
        }
    }
}

/// Échappe `&`, `<`, `>` et `"` pour un contenu ou un attribut XML.
pub fn escape_into(buffer: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => buffer.push_str("&amp;"),
            '<' => buffer.push_str("&lt;"),
            '>' => buffer.push_str("&gt;"),
            '"' => buffer.push_str("&quot;"),
            other => buffer.push(other),
        }
    }
}

/// Échappe un texte pour un contenu XML (les guillemets restent lisibles).
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}
