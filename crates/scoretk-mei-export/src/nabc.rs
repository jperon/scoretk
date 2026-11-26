// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : écriture de la seconde portée manuscrite (NABC)
// en neumes MEI 5.1, d'après les *Guidelines* ch. 6 (§6.1, §6.1.1,
// §6.4.1–§6.4.9), le schéma `mei-all-5.1.rng` (ECL-2.0) et la référence
// Gregorio `doc/GregorioNabcRef.tex` (GPL-3.0-or-later), rédigé en 2026.
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

//! Seconde portée adiastématique : une `<staff>` parallèle, déclarée sur
//! place par un `<staffDef>` enfant sans `@lines`, où chaque snippet NABC
//! devient un ou plusieurs `<neume>` structurés.
//!
//! Le texte exact du snippet reste dans un `<signifLet type="gabc-nabc">`
//! (`Guidelines` §6.4.1/§6.4.9 ; RNG `mei_signifLet` accepte du texte libre) :
//! c'est le porteur de la fidélité GABC → MEI → GABC, indépendamment de
//! l'interprétation sémiologique des glyphes. Les deux `<syllable>` d'une
//! paire se lient par `@xml:id`/`@corresp` (RNG `mei_att.linking.attribute.
//! corresp`). Rien n'est émis quand le chant n'a pas de NABC : le chemin
//! CMN/neumes habituel reste inchangé.

use scoretk_core::model::Gregorian;
use scoretk_mei_common::{ids, nabc, neumes};
use scoretk_xml_common::XmlWriter;

/// Vrai si au moins une syllabe porte un snippet NABC.
pub(super) fn has_nabc(gregorian: &Gregorian) -> bool {
    gregorian
        .syllables
        .iter()
        .any(|syllable| !syllable.nabc.is_empty())
}

/// `xml:id` de la syllabe de la portée demandée (1 = carrée, 2 = manuscrite).
pub(super) fn syllable_id(index: usize, staff: u8) -> String {
    if staff == 1 {
        format!("s{}a", index + 1)
    } else {
        format!("s{}b", index + 1)
    }
}

/// Attributs de lien d'une syllabe : identifiant stable et `@corresp`
/// vers l'autre portée (`data.URI`, RNG :21213).
pub(super) fn link(index: usize, staff: u8) -> Vec<(String, String)> {
    let other = if staff == 1 { 2 } else { 1 };
    vec![
        ("xml:id".to_owned(), syllable_id(index, staff)),
        (
            "corresp".to_owned(),
            format!("#{}", syllable_id(index, other)),
        ),
    ]
}

/// `<scoreDef>` de la portée carrée seulement. La portée manuscrite est
/// déclarée dans son propre `<staff>` (voir `write_staff`) : `mei_staff` accepte
/// `<staffDef>` comme enfant (RNG :28007) et la contrainte Schematron
/// `mei-staff-checkStaff_n` (:28025–28031) reconnaît explicitement ce
/// « staffDef child element ». Les Guidelines §4.2.2 permettent `staffDef` dans une
/// portée avec le MEI All schema (déconseillé seulement pour le CMN), et le
/// RNG définit `<staff>` comme couvrant « diastematic and non-diastematic
/// signs ». Aucun `@lines` n'est donc inventé pour le manuscrit adiastématique
/// (`lines="0"` serait invalide, `positiveInteger`) ; voir `mei.md` §12.3.1.
pub(super) fn write_score_def(xml: &mut XmlWriter, gregorian: &Gregorian, initial_clefs: usize) {
    xml.start_element("scoreDef", &[]);
    xml.start_element("staffGrp", &[]);
    xml.start_element(
        "staffDef",
        &[
            ("n", "1"),
            ("lines", &gregorian.staff_lines.clamp(2, 5).to_string()),
            ("notationtype", "neume"),
            ("label", "square"),
        ],
    );
    if let Some(token) = gregorian.clefs.iter().take(initial_clefs).next() {
        if let Some(parts) = neumes::clef_parts(token) {
            let shape = parts.shape.to_string();
            let line = parts.line.to_string();
            let mut attributes = vec![("shape", shape.as_str()), ("line", line.as_str())];
            if neumes::clef_is_flat(token) {
                attributes.push(("type", "clef-flat"));
            }
            xml.tag_empty("clef", &attributes);
        }
    }
    xml.end_element();
    xml.end_element();
    xml.end_element();
}

/// Écrit la portée manuscrite complète : un `<syllable>` par syllabe du
/// chant, pour garder l'alignement avec la portée carrée via `@corresp`.
///
/// Le `<staffDef>` est un enfant de la portée (Guidelines §4.2.2, MEI All) :
/// il porte `notationtype`/`label` sans `@lines`, ce qui satisfait à la fois
/// la RNG et la contrainte Schematron `staffDef-Check_staffDefn`
/// (`mei.md` §12.3.1). Les lecteurs tiers et le MWE historique sans ce
/// `<staffDef>` restent acceptés par le lecteur de ce port.
pub(super) fn write_staff(xml: &mut XmlWriter, gregorian: &Gregorian) {
    xml.start_element("staff", &[("n", "2")]);
    xml.tag_empty(
        "staffDef",
        &[
            ("n", "2"),
            ("notationtype", "neume"),
            ("label", "manuscript"),
        ],
    );
    xml.start_element("layer", &[("n", "1")]);
    for (index, syllable) in gregorian.syllables.iter().enumerate() {
        let links = link(index, 2);
        let refs: Vec<(&str, &str)> = links
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        xml.start_element("syllable", &refs);
        xml.tag_with("syl", &[("con", "s")], &syllable.text);
        for snippet in &syllable.nabc {
            for descriptor in nabc::parse(&snippet.text) {
                write_descriptor(xml, &descriptor);
            }
            // Le texte NABC atomique est le porteur exact du round trip ;
            // `@n` garde l'ordre des snippets dans la syllabe. Le slash qui
            // ouvre le segment carré après la contre-coupure est un espacement
            // source : `@label` (attribut MEI standard) le transporte sans
            // annotation privée.
            let notes_before = snippet.notes_before.to_string();
            let mut attributes = vec![
                ("type", "gabc-nabc"),
                // `@n` porte le nombre de notes carrées avant la coupure :
                // c'est la donnée nécessaire pour reconstruire
                // l'interleavage GABC exact à la lecture.
                ("n", notes_before.as_str()),
            ];
            let resume = neumes::layout_label("resume", &snippet.resume);
            if !snippet.resume.is_empty() {
                attributes.push(("label", resume.as_str()));
            }
            xml.tag_with("signifLet", &attributes, &snippet.text);
        }
        xml.end_element();
    }
    xml.end_element();
    xml.end_element();
}

fn write_descriptor(xml: &mut XmlWriter, descriptor: &nabc::Descriptor) {
    if !descriptor.spacing.is_empty() {
        // L'espacement horizontal NABC n'a pas d'élément MEI dédié : il est
        // porté par le texte du snippet, pas simulé par un `<space>` qui
        // serait lu comme un silence par un lecteur tiers.
        return;
    }
    if descriptor.glyphs.is_empty() {
        return;
    }
    let name = descriptor.glyphs[0].name;
    let intm: Vec<Option<&str>> = descriptor
        .glyphs
        .iter()
        .scan('f', |previous, glyph| {
            let height = glyph.height.unwrap_or('f');
            let direction = if glyph.height.is_some() {
                Some(nabc::interval(*previous, height))
            } else {
                None
            };
            *previous = height;
            Some(direction)
        })
        .collect();
    xml.start_element("neume", &[("type", name)]);
    let mut nc_index = 0;
    for glyph in &descriptor.glyphs {
        for _ in 0..glyph.components {
            let mut attributes: Vec<(&str, String)> = Vec::new();
            if let Some(direction) = intm.get(nc_index).copied().flatten() {
                attributes.push(("intm", direction.to_owned()));
            }
            let refs: Vec<(&str, &str)> = attributes
                .iter()
                .map(|(key, value)| (*key, value.as_str()))
                .collect();
            xml.start_element("nc", &refs);
            if nabc::is_quilisma(glyph.name) {
                let waves = nabc::quilisma_waves(&glyph.code).to_string();
                xml.tag_empty("quilisma", &[("waves", &waves)]);
            }
            if glyph.modifiers.contains('-') {
                xml.tag_empty("episema", &[("form", "h"), ("place", "above")]);
            }
            if glyph.modifiers.contains('>') {
                xml.tag_empty("liquescent", &[("curve", "c")]);
            }
            if glyph.modifiers.contains('~') {
                xml.tag_empty("liquescent", &[("curve", "a")]);
            }
            xml.end_element();
            nc_index += 1;
        }
    }
    for (rising, count) in &descriptor.puncta {
        for _ in 0..(*count).min(8) {
            xml.tag_empty("nc", &[("intm", if *rising { "u" } else { "d" })]);
        }
    }
    for (shorthand, position) in &descriptor.letters {
        let mut attributes = vec![("type", "gabc-nabc-letter".to_owned())];
        if let Some(place) = nabc::place(*position) {
            attributes.push(("place", place.to_owned()));
        }
        attributes.push(("n", position.to_string()));
        let refs: Vec<(&str, &str)> = attributes
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        xml.tag_with("signifLet", &refs, shorthand);
    }
    xml.end_element();
    let _ = ids::MEI_NAMESPACE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Gregorian, NabcSnippet, Syllable};
    use scoretk_xml_common::XmlWriter;

    fn chant_with_resume(resume: &str) -> Gregorian {
        Gregorian {
            syllables: vec![Syllable {
                nabc: vec![NabcSnippet {
                    notes_before: 1,
                    text: "ds-".to_owned(),
                    resume: resume.to_owned(),
                }],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        }
    }

    #[test]
    fn the_cut_resumption_uses_a_standard_label_not_a_private_annotation() {
        let mut xml = XmlWriter::new();
        write_staff(&mut xml, &chant_with_resume("/"));
        let output = xml.finish();
        assert!(
            output.contains("type=\"gabc-nabc\" n=\"1\" label=\"layout:resume:U002F\">ds-</signifLet>")
        );
        assert!(!output.contains("<annot"));

        let mut xml = XmlWriter::new();
        write_staff(&mut xml, &chant_with_resume(""));
        let output = xml.finish();
        assert!(!output.contains("layout:resume"));
    }
}
