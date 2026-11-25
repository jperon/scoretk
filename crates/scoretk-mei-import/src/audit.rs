// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : instrumentation de la couverture d'import MEI.
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

//! Audit d'import : quels éléments et attributs MEI 5.1 le lecteur utilise ?
//!
//! Le port de MuseScore est un analyseur « best effort » : un élément non
//! reconnu est ignoré sans erreur. Cette instrumentation comptabilise, pour
//! chaque document, les éléments et attributs *présents* et ceux que le
//! lecteur **ne mappe pas** vers le modèle pivot. Les ensembles ci-dessous
//! décrivent exactement les noms consommés par `reader.rs`, `header.rs`,
//! `controls.rs`, `barlines.rs` et `convert.rs` ; ils doivent être mis à jour
//! avec le lecteur (MEI 5.1 `MEI.shared.xml`, `att.*`, et les conventions
//! `mscore-*` de MuseScore 4.7.4).

use std::collections::{BTreeMap, BTreeSet};

use scoretk_xml_common::Element;

use crate::Error;

/// Éléments MEI explicitement consommés par le lecteur. Les conteneurs
/// structurels (`mei`, `music`, `body`, `mdiv`, `score`, `section`, `staff`,
/// `layer`, `measure`) sont comptés comme mappés : le lecteur les parcourt.
const MAPPED_ELEMENTS: &[&str] = &[
    // Structure (MeiImporter::readScore / readSectionElements).
    "mei",
    "meiHead",
    "fileDesc",
    "titleStmt",
    "title",
    "persName",
    "composer",
    "lyricist",
    "arranger",
    "pubStmt",
    "date",
    "availability",
    "encodingDesc",
    "sourceDesc",
    "revisionDesc",
    "music",
    "body",
    "mdiv",
    "score",
    "scoreDef",
    "staffGrp",
    "staffDef",
    "instrDef",
    "section",
    "measure",
    "staff",
    "layer",
    "ending",
    "pb",
    "sb",
    "pgHead",
    "rend",
    "hi",
    "lb",
    "label",
    "labelAbbr",
    // Événements et notations (readElements / readControlEvents).
    "beam",
    "chord",
    "note",
    "rest",
    "space",
    "mRest",
    "mRpt",
    "tuplet",
    "clef",
    "meterSig",
    "keySig",
    "bTrem",
    "fTrem",
    "graceGrp",
    "accid",
    "artic",
    "arpeg",
    "fermata",
    "fing",
    "dir",
    "dynam",
    "reh",
    "harm",
    "trill",
    "mordent",
    "turn",
    "ornam",
    "tie",
    "slur",
    "phrase",
    "tempo",
    "hairpin",
    "pedal",
    "octave",
    "breath",
    "caesura",
    "gliss",
    "repeatMark",
    "trill-spanner",
    "verse",
    "syl",
];

/// Attributs MEI 5.1 (et extensions `mscore-*` documentées) lus par le port.
/// La clé est le nom local : `xml:id` est déjà ramené à `id` par l'arbre XML.
const MAPPED_ATTRIBUTES: &[&str] = &[
    "accid",
    "accid.ges",
    "artic",
    "bar.thru",
    "bracket.place",
    "bracket.visible",
    "breaksec",
    "clef.dis",
    "clef.dis.place",
    "clef.line",
    "clef.shape",
    "count",
    "dis",
    "dis.place",
    "dots",
    "dots.ges",
    "dur",
    "dur.ges",
    "enclose",
    "endid",
    "fontsize",
    "form",
    "func",
    "glyph.auth",
    "glyph.name",
    "halign",
    "head.shape",
    "id",
    "isodate",
    "keysig",
    "label",
    "left",
    "line",
    "lines",
    "lines.visible",
    "metcon",
    "meter.count",
    "meter.sym",
    "meter.unit",
    "midi.bpm",
    "midi.channel",
    "midi.instrnum",
    "mm",
    "n",
    "num",
    "numbase",
    "oct",
    "optimize",
    "page.botmar",
    "page.height",
    "page.leftmar",
    "page.rightmar",
    "page.topmar",
    "page.width",
    "pan",
    "place",
    "pname",
    "right",
    "role",
    "scale",
    "shape",
    "sig",
    "staff",
    "startid",
    "stem.dir",
    "sym",
    "symbol",
    "trans.diat",
    "trans.semi",
    "tstamp",
    "tstamp2",
    "type",
    "unit",
    "valign",
    "vel",
    "visible",
    "volume",
    "vu.height",
];

/// Déclarations XML, pas des attributs musicaux MEI ; elles ne sont pas
/// comptabilisées comme des pertes d'import.
const XML_DECLARATIONS: &[&str] = &[
    "xmlns",
    "xmlns:xlink",
    "xmlns:mei",
    "xlink",
    "schemaLocation",
];

/// Rapport d'import d'un document MEI, par nom d'élément et par couple
/// `(élément, attribut)`. Les compteurs `present_*` donnent le dénominateur
/// (ce qui est dans la source) et les compteurs `ignored_*` le numérateur des
/// pertes d'information du lecteur.
#[derive(Clone, Debug, Default)]
pub struct ImportAudit {
    present_elements: BTreeMap<String, usize>,
    ignored_elements: BTreeMap<String, usize>,
    present_attributes: BTreeMap<(String, String), usize>,
    ignored_attributes: BTreeMap<(String, String), usize>,
}

impl ImportAudit {
    /// Nombre d'éléments présents dans le document.
    pub fn present_element_total(&self) -> usize {
        self.present_elements.values().sum()
    }

    /// Nombre d'éléments que le lecteur n'utilise pas.
    pub fn ignored_element_total(&self) -> usize {
        self.ignored_elements.values().sum()
    }

    /// Nombre d'attributs présents dans le document.
    pub fn present_attribute_total(&self) -> usize {
        self.present_attributes.values().sum()
    }

    /// Nombre d'attributs que le lecteur n'utilise pas.
    pub fn ignored_attribute_total(&self) -> usize {
        self.ignored_attributes.values().sum()
    }

    /// Éléments ignorés, du plus fréquent au plus rare.
    pub fn ignored_elements(&self) -> Vec<(&str, usize)> {
        let mut rows: Vec<_> = self
            .ignored_elements
            .iter()
            .map(|(name, count)| (name.as_str(), *count))
            .collect();
        rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        rows
    }

    /// Attributs ignorés, du plus fréquent au plus rare.
    pub fn ignored_attributes(&self) -> Vec<(&str, &str, usize)> {
        let mut rows: Vec<_> = self
            .ignored_attributes
            .iter()
            .map(|((element, attribute), count)| (element.as_str(), attribute.as_str(), *count))
            .collect();
        rows.sort_by(|left, right| {
            right
                .2
                .cmp(&left.2)
                .then_with(|| left.0.cmp(right.0).then_with(|| left.1.cmp(right.1)))
        });
        rows
    }

    /// Ajoute les compteurs d'un autre document (rapport agrégé du lot).
    pub fn merge(&mut self, other: &Self) {
        for (key, value) in &other.present_elements {
            *self.present_elements.entry(key.clone()).or_insert(0) += value;
        }
        for (key, value) in &other.ignored_elements {
            *self.ignored_elements.entry(key.clone()).or_insert(0) += value;
        }
        for (key, value) in &other.present_attributes {
            *self.present_attributes.entry(key.clone()).or_insert(0) += value;
        }
        for (key, value) in &other.ignored_attributes {
            *self.ignored_attributes.entry(key.clone()).or_insert(0) += value;
        }
    }
}

/// Audit d'un document MEI en mémoire, sans le convertir : les noms sont
/// comparés à ceux que le lecteur consomme effectivement.
pub fn audit_mei(bytes: &[u8]) -> Result<ImportAudit, Error> {
    let root = scoretk_xml_common::parse(bytes)?;
    let mut audit = ImportAudit::default();
    visit(&root, &mut audit);
    Ok(audit)
}

fn visit(node: &Element, audit: &mut ImportAudit) {
    *audit.present_elements.entry(node.name.clone()).or_insert(0) += 1;
    if !MAPPED_ELEMENTS.contains(&node.name.as_str()) {
        *audit.ignored_elements.entry(node.name.clone()).or_insert(0) += 1;
    }
    for (key, value) in &node.attributes {
        if XML_DECLARATIONS.contains(&key.as_str()) {
            continue;
        }
        let pair = (node.name.clone(), key.clone());
        *audit.present_attributes.entry(pair.clone()).or_insert(0) += 1;
        if !MAPPED_ATTRIBUTES.contains(&key.as_str()) || value.is_empty() {
            *audit.ignored_attributes.entry(pair).or_insert(0) += 1;
        }
    }
    for child in &node.children {
        visit(child, audit);
    }
}

/// Noms d'éléments comptés comme mappés : exposés pour que les tests puissent
/// vérifier que l'audit et le lecteur ne divergent pas silencieusement.
pub fn mapped_element_names() -> BTreeSet<&'static str> {
    MAPPED_ELEMENTS.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_mapped_and_ignored_elements() {
        let bytes = br#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
          <music><body><mdiv><score>
            <scoreDef><staffGrp><staffDef n="1" lines="5"/></staffGrp></scoreDef>
            <section><measure n="1"><staff n="1"><layer n="1">
              <note xml:id="n1" dur="4" pname="c" oct="4" head.shape="diamond">
                <artic artic="stacc"/>
              </note>
              <tupletSpan startid="n1" num="3" numbase="2"/>
            </layer></staff></measure></section>
          </score></mdiv></body></music></mei>"#;
        let audit = audit_mei(bytes).expect("audit");
        assert_eq!(audit.present_element_total(), 15);
        // `tupletSpan` n'est pas encore lu : c'est une perte mesurée.
        assert_eq!(audit.ignored_element_total(), 1);
        assert_eq!(audit.ignored_elements(), [("tupletSpan", 1)]);
        for name in ["score", "measure", "note", "artic", "syl"] {
            assert!(mapped_element_names().contains(name), "{name} is mapped");
        }
        assert!(!mapped_element_names().contains("tupletSpan"));
        assert!(!audit
            .ignored_attributes()
            .contains(&("note", "head.shape", 1)));
        assert!(audit
            .ignored_attributes()
            .iter()
            .any(|(element, attribute, _)| *element == "mei" && *attribute == "meiversion"));
    }
}
