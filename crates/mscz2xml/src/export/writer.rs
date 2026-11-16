//! Écriture XML indentée, calquée sur `muse::XmlStreamWriter`.
//!
//! MuseScore écrit chaque élément sur sa propre ligne, avec deux espaces par
//! niveau. Particularité reproduite ici : la balise fermante est écrite au
//! niveau d'indentation des enfants, c'est-à-dire un cran plus loin que la
//! balise ouvrante. Les fichiers `.ref.musicxml` du corpus ont exactement cette
//! forme, ce qui permet de comparer les sorties ligne à ligne.
//!
//! Le tampon est une `String` : aucune entrée/sortie, le module reste
//! compilable pour `wasm32-unknown-unknown`.

/// Écriture XML minimale et déterministe.
pub(crate) struct XmlWriter {
    buffer: String,
    indent: usize,
    open_elements: Vec<String>,
}

impl XmlWriter {
    /// Construit un écrivain vide.
    pub(crate) fn new() -> Self {
        Self {
            buffer: String::new(),
            indent: 0,
            open_elements: Vec::new(),
        }
    }

    /// Termine le document et renvoie son contenu.
    ///
    /// Le document se termine toujours par un saut de ligne.
    pub(crate) fn finish(mut self) -> String {
        if !self.buffer.is_empty() && !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
        self.buffer
    }

    /// Écrit la déclaration `<?xml version="1.0" encoding="UTF-8"?>`.
    pub(crate) fn start_document(&mut self) {
        self.buffer
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    }

    /// Écrit une déclaration `DOCTYPE`.
    pub(crate) fn doctype(&mut self, declaration: &str) {
        self.start_line();
        self.buffer.push_str("<!DOCTYPE ");
        self.buffer.push_str(declaration);
        self.buffer.push('>');
    }

    /// Ouvre un élément et augmente l'indentation.
    pub(crate) fn start_element(&mut self, name: &str, attributes: &[(&str, &str)]) {
        self.start_line();
        self.buffer.push('<');
        self.buffer.push_str(name);
        self.write_attributes(attributes);
        self.buffer.push('>');
        self.indent += 1;
        self.open_elements.push(name.to_owned());
    }

    /// Ferme l'élément ouvert en dernier.
    ///
    /// # Panics
    ///
    /// Panique si aucun élément n'est ouvert.
    pub(crate) fn end_element(&mut self) {
        let name = self
            .open_elements
            .pop()
            .expect("fermeture d'un élément XML inexistant");
        self.start_line();
        self.buffer.push_str("</");
        self.buffer.push_str(&name);
        self.buffer.push('>');
        self.indent -= 1;
    }

    /// Écrit un élément vide auto-fermant avec attributs (`<x a="b"/>`).
    pub(crate) fn tag_empty(&mut self, name: &str, attributes: &[(&str, &str)]) {
        self.start_line();
        self.buffer.push('<');
        self.buffer.push_str(name);
        self.write_attributes(attributes);
        self.buffer.push_str("/>");
    }

    /// Écrit un élément vide sans attribut (`<x/>`).
    pub(crate) fn tag(&mut self, name: &str, body: &str) {
        self.tag_with(name, &[], body);
    }

    /// Écrit un élément avec attributs et contenu sur une seule ligne.
    pub(crate) fn tag_with(&mut self, name: &str, attributes: &[(&str, &str)], body: &str) {
        self.start_line();
        self.buffer.push('<');
        self.buffer.push_str(name);
        self.write_attributes(attributes);
        self.buffer.push('>');
        escape_text_into(&mut self.buffer, body);
        self.buffer.push_str("</");
        self.buffer.push_str(name);
        self.buffer.push('>');
    }

    fn start_line(&mut self) {
        if !self.buffer.is_empty() {
            self.buffer.push('\n');
        }
        for _ in 0..self.indent {
            self.buffer.push_str("  ");
        }
    }

    fn write_attributes(&mut self, attributes: &[(&str, &str)]) {
        for (name, value) in attributes {
            self.buffer.push(' ');
            self.buffer.push_str(name);
            self.buffer.push_str("=\"");
            escape_attribute_into(&mut self.buffer, value);
            self.buffer.push('"');
        }
    }
}

/// Échappe le contenu textuel d'un élément.
///
/// MuseScore échappe aussi les guillemets doubles (`XmlWriter::escape`) ; le
/// corpus en contient dans des crédits et des paroles.
fn escape_text_into(buffer: &mut String, text: &str) {
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

/// Échappe la valeur d'un attribut (guillemets doubles compris).
fn escape_attribute_into(buffer: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => buffer.push_str("&amp;"),
            '<' => buffer.push_str("&lt;"),
            '>' => buffer.push_str("&gt;"),
            '"' => buffer.push_str("&quot;"),
            '\'' => buffer.push_str("&apos;"),
            other => buffer.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indentation_comme_musescore() {
        let mut xml = XmlWriter::new();
        xml.start_document();
        xml.doctype("score-partwise");
        xml.start_element("score-partwise", &[("version", "4.0")]);
        xml.start_element("work", &[]);
        xml.tag("work-title", "Titre");
        xml.end_element();
        xml.tag_empty("divisions", &[("x", "1")]);
        xml.end_element();

        let expected = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE score-partwise>\n<score-partwise version=\"4.0\">\n  <work>\n    <work-title>Titre</work-title>\n    </work>\n  <divisions x=\"1\"/>\n  </score-partwise>\n";
        assert_eq!(xml.finish(), expected);
    }

    #[test]
    fn echappement() {
        let mut xml = XmlWriter::new();
        xml.tag("words", "a & b < c");
        xml.tag("words", "dit \"bonjour\"");
        xml.tag_empty("x", &[("v", "\"quoted\"")]);
        assert_eq!(
            xml.finish(),
            "<words>a &amp; b &lt; c</words>\n\
             <words>dit &quot;bonjour&quot;</words>\n\
             <x v=\"&quot;quoted&quot;\"/>\n"
        );
    }
}
