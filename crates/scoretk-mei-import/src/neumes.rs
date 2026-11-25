// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture du module NEUMES de MEI 5.1 vers le flux
// L6 grégorien, d'après les *Guidelines* 5.1 ch. 6 (§6.1, §6.1.1,
// §6.4.1–§6.4.10) et `mei-all-5.1.rng`, rédigé en 2026.
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

//! Lecture neumatique : reconstruction du payload L6 à partir de
//! `<syllable>`/`<syl>`/`<neume>`/`<nc>` (`Guidelines` §6.4.10), puis
//! reconstitution de la vue moderne dérivée par
//! `Gregorian::build_derived_score`. Les extensions atomiques `gabc-*` sont
//! préférées quand elles sont présentes ; en leur absence, la lecture se
//! base uniquement sur les éléments MEI 5.1 standard.

use std::collections::BTreeMap;

use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, NeumeSign, Score, SignKind,
    Syllable,
};
use scoretk_mei_common::{neumes, Error};
use scoretk_xml_common::Element;

/// Lit un document MEI 5.1 dont la partition contient des neumes.
pub(super) fn read(root: &Element) -> Result<Score, Error> {
    let mut gregorian = Gregorian::default();
    read_headers(root, &mut gregorian);
    read_staff_lines(root, &mut gregorian);

    let Some(layer) = find_first(root, "layer") else {
        return Err(Error::Unsupported("couche neumatique absente".to_owned()));
    };
    initial_clefs(root, layer, &mut gregorian);
    let mut current_clef = gregorian
        .clefs
        .first()
        .cloned()
        .unwrap_or_else(|| "c4".to_owned());

    for syllable_node in layer.children.iter().filter(|node| node.name == "syllable") {
        let mut syllable = Syllable::default();
        if let Some(syl) = syllable_node.child("syl") {
            // Le texte exact est conservé (espaces de bord compris) :
            // `text_trimmed()` supprimerait les espaces de mot de GABC.
            syllable.text = syl.text.clone();
        }
        for annot in syllable_node
            .children
            .iter()
            .filter(|node| node.name == "annot")
        {
            let (kind, _) = annot_type(annot);
            match kind {
                neumes::ABOVE_ANNOT => {
                    syllable.above_text = Some(annot.text.clone());
                }
                neumes::RUBRIC_ANNOT => {
                    syllable.rubric = Some(annot.text.clone());
                }
                neumes::SPACE_ANNOT => {
                    syllable.space_after = annot.text.trim() != "0" && !annot.text.is_empty();
                }
                _ => {}
            }
        }

        // Une clé sur la syllabe est un changement en cours de chant
        // (`Gabc.tex:847-870`) ; elle entre dans la liste source des clés.
        let clef_annot = syllable_annot(syllable_node, neumes::CLEF_ANNOT).next();
        if let Some(annot) = clef_annot {
            syllable.clef = Some(annot.text.clone());
            gregorian.clefs.push(annot.text.clone());
            current_clef = annot.text.clone();
        } else if let Some(clef) = syllable_node.child("clef") {
            let token = clef_token(clef);
            syllable.clef = Some(token.clone());
            gregorian.clefs.push(token.clone());
            current_clef = token;
        }

        let note_anotts = syllable_annot(syllable_node, neumes::NOTE_ANNOT)
            .filter_map(|annot| Some((annot.attribute("n")?.parse::<usize>().ok()?, annot)))
            .collect::<BTreeMap<_, _>>();
        let source_anotts = syllable_annot(syllable_node, neumes::SOURCE_ANNOT)
            .filter_map(|annot| Some((annot.attribute("n")?.parse::<usize>().ok()?, annot)))
            .collect::<BTreeMap<_, _>>();
        let sign_anotts = sign_anotts(syllable_node);
        let bar_anotts = syllable_annot(syllable_node, neumes::BAR_ANNOT)
            .filter_map(|annot| Some((annot.attribute("n")?.parse::<usize>().ok()?, annot)))
            .collect::<BTreeMap<_, _>>();

        for neume in syllable_node
            .children
            .iter()
            .filter(|node| node.name == "neume")
        {
            for nc in neume.children.iter().filter(|node| node.name == "nc") {
                let index = syllable.notes.len();
                let mut note = nc_note(nc, &current_clef, note_anotts.get(&index));
                note.signs = nc_signs(nc, index, &sign_anotts);
                if let Some(annot) = source_anotts.get(&index) {
                    let mut parts = annot.text.split(':');
                    note.source_offset = parts.next().and_then(|value| value.parse().ok());
                    note.source_end = parts.next().and_then(|value| value.parse().ok());
                }
                syllable.notes.push(note);
            }
        }

        let mut bar_index = 0;
        for node in &syllable_node.children {
            if node.name == "divLine" {
                let bar = bar_anotts
                    .get(&bar_index)
                    .and_then(|annot| BarKind::parse_prefix(&annot.text).map(|(bar, _)| bar))
                    .unwrap_or_else(|| neumes::form_bar(node.attribute("form").unwrap_or("")));
                syllable.bars.push(bar);
                bar_index += 1;
            }
        }

        let mut group_signs: Vec<(usize, NeumeSign)> = Vec::new();
        for node in &syllable_node.children {
            if node.name == "episema" {
                let raw = match node.attribute("form") {
                    Some("v") => "'",
                    _ => "_",
                };
                group_signs.push((group_signs.len(), NeumeSign::new(episema_kind(raw), raw)));
            }
        }
        for (key, annot) in &sign_anotts {
            if let Some((prefix, index)) = key.split_once(':') {
                if prefix.is_empty() && index.parse::<usize>().is_ok() {
                    let index = index.parse::<usize>().unwrap_or(0);
                    let sign = sign_from_annot(annot);
                    if let Some((_, existing)) = group_signs.iter_mut().find(|(i, _)| *i == index) {
                        *existing = sign;
                    } else {
                        group_signs.push((index, sign));
                    }
                }
            }
        }
        group_signs.sort_by_key(|(index, _)| *index);
        syllable.signs = group_signs.into_iter().map(|(_, sign)| sign).collect();

        gregorian.syllables.push(syllable);
    }
    let mut score = gregorian.build_derived_score();
    apply_metadata(&mut score, &gregorian);
    gregorian.derived_modern_view = true;
    score.extensions.format = Some(Extension::Gregorian(gregorian));
    Ok(score)
}

/// Parcours récursif limité aux documents MEI (aucune entrée/sortie).
fn find_first<'a>(node: &'a Element, name: &str) -> Option<&'a Element> {
    if node.name == name {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_first(child, name))
}

fn collect<'a>(node: &'a Element, name: &str, out: &mut Vec<&'a Element>) {
    if node.name == name {
        out.push(node);
    }
    for child in &node.children {
        collect(child, name, out);
    }
}

/// `@type` d'une annotation : le premier jeton est le genre, le second
/// (quand il existe) la famille de signe GABC.
fn annot_type(node: &Element) -> (&str, Option<&str>) {
    let mut parts = node.attribute("type").unwrap_or("").splitn(2, ' ');
    (parts.next().unwrap_or(""), parts.next())
}

fn syllable_annot<'a>(
    syllable: &'a Element,
    kind: &'a str,
) -> impl Iterator<Item = &'a Element> + 'a {
    syllable
        .children
        .iter()
        .filter(move |node| node.name == "annot" && annot_type(node).0 == kind)
}

fn sign_anotts(syllable: &Element) -> BTreeMap<String, &Element> {
    let mut result = BTreeMap::new();
    for node in &syllable.children {
        if node.name != "annot" {
            continue;
        }
        let (kind, _) = annot_type(node);
        if kind == neumes::NOTE_SIGN_ANNOT || kind == neumes::GROUP_SIGN_ANNOT {
            if let Some(n) = node.attribute("n") {
                result.insert(n.to_owned(), node);
            }
        }
    }
    result
}

fn sign_from_annot(annot: &Element) -> NeumeSign {
    let (_, family) = annot_type(annot);
    NeumeSign::new(
        family.map_or(SignKind::Other, neumes::sign_kind_from_name),
        annot.text.clone(),
    )
}

fn read_headers(root: &Element, gregorian: &mut Gregorian) {
    let mut anotts = Vec::new();
    collect(root, "annot", &mut anotts);
    for annot in anotts {
        let (kind, _) = annot_type(annot);
        if kind == neumes::HEADER_ANNOT {
            if let Some(name) = annot.attribute("n") {
                gregorian.headers.push(GregorianHeader {
                    name: name.to_owned(),
                    value: annot.text.clone(),
                });
            }
        }
    }
    if gregorian.headers.is_empty() {
        if let Some(title) = find_first(root, "title") {
            gregorian.headers.push(GregorianHeader {
                name: "name".to_owned(),
                value: title.text_trimmed().to_owned(),
            });
        }
    }
}

fn read_staff_lines(root: &Element, gregorian: &mut Gregorian) {
    let mut defs = Vec::new();
    collect(root, "staffDef", &mut defs);
    gregorian.staff_lines = defs
        .first()
        .and_then(|def| def.attribute("lines"))
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(4)
        .clamp(2, 5);
}

/// Clés placées avant la première syllabe, sous forme de jetons GABC.
fn initial_clefs(root: &Element, layer: &Element, gregorian: &mut Gregorian) -> usize {
    let mut tokens: Vec<(usize, String)> = Vec::new();
    for annot in layer.children.iter().filter(|node| node.name == "annot") {
        let (kind, _) = annot_type(annot);
        if kind == neumes::CLEF_ANNOT {
            let index = annot
                .attribute("n")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(tokens.len());
            tokens.push((index, annot.text.clone()));
        }
    }
    tokens.sort_by_key(|(index, _)| *index);
    if tokens.is_empty() {
        // Seule la clé du `<scoreDef>` est initiale : une clé portée par une
        // syllabe est un changement en cours de chant et ne doit pas être
        // ajoutée deux fois au flux L6.
        if let Some(score_def) = find_first(root, "scoreDef") {
            if let Some(clef) = find_first(score_def, "clef") {
                gregorian.clefs.push(clef_token(clef));
            }
        }
    } else {
        gregorian
            .clefs
            .extend(tokens.into_iter().map(|(_, token)| token));
    }
    gregorian.clefs.len()
}

fn clef_token(clef: &Element) -> String {
    neumes::clef_token(
        clef.attribute("shape").unwrap_or("C"),
        clef.attribute("line")
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(3),
    )
}

/// Une note GABC reconstruite depuis `<nc>` et ses annotations.
fn nc_note(nc: &Element, clef: &str, private: Option<&&Element>) -> GregorianNote {
    if let Some(annot) = private {
        let mut chars = annot.text.chars();
        if let Some(letter) = chars.next() {
            return GregorianNote {
                letter,
                glyph: chars.collect(),
                ..GregorianNote::default()
            };
        }
    }
    let letter = neumes::nc_letter(nc.attribute("pname"), parse_oct(nc), clef).unwrap_or('g');
    let mut glyph = String::new();
    for child in &nc.children {
        let shape = match child.name.as_str() {
            "liquescent" => "~",
            "oriscus" => "o",
            "quilisma" => "w",
            "strophicus" => "s",
            "episema" => match child.attribute("form") {
                Some("v") => "'",
                _ => "_",
            },
            _ => continue,
        };
        glyph.push_str(shape);
    }
    // Les signes sont reconstruits par `nc_signs` : cette fonction ne garde
    // que la lettre et le glyphe, qui sont les champs comparés de l'inventaire.
    GregorianNote {
        letter,
        glyph,
        ..GregorianNote::default()
    }
}

fn parse_oct(nc: &Element) -> Option<u8> {
    nc.attribute("oct")?.parse().ok()
}

/// Signes d'une note : jetons exacts quand ils sont présents, sinon éléments
/// MEI 5.1 standard.
fn nc_signs(
    nc: &Element,
    note_index: usize,
    sign_anotts: &BTreeMap<String, &Element>,
) -> Vec<NeumeSign> {
    let prefix = format!("{note_index}:");
    let private: Vec<_> = sign_anotts
        .iter()
        .filter_map(|(key, annot)| {
            let suffix = key.strip_prefix(&prefix)?;
            let index = suffix.parse::<usize>().ok()?;
            Some((index, *annot))
        })
        .collect();
    if !private.is_empty() {
        let mut result = private;
        result.sort_by_key(|(index, _)| *index);
        return result
            .into_iter()
            .map(|(_, annot)| sign_from_annot(annot))
            .collect();
    }
    let mut signs = Vec::new();
    for child in &nc.children {
        let (raw, kind) = match child.name.as_str() {
            "liquescent" => ("~", SignKind::Liquescentia),
            "oriscus" => ("o", SignKind::Oriscus),
            "quilisma" => ("w", SignKind::Quilisma),
            "strophicus" => ("s", SignKind::Stropha),
            "episema" => match child.attribute("form") {
                Some("v") => ("'", SignKind::Ictus),
                _ => ("_", SignKind::HorizontalEpisema),
            },
            _ => continue,
        };
        signs.push(NeumeSign::new(kind, raw));
    }
    signs
}

fn episema_kind(raw: &str) -> SignKind {
    if raw == "'" {
        SignKind::Ictus
    } else {
        SignKind::HorizontalEpisema
    }
}

/// Copie des en-têtes interprétés dans les métadonnées communes, comme le
/// fait le lecteur GABC (`scoretk-gabc-import/src/read.rs:61-78`).
fn apply_metadata(score: &mut Score, gregorian: &Gregorian) {
    for header in &gregorian.headers {
        match header.name.as_str() {
            "name" => {
                score.metadata.work_title = header.value.clone();
                score.metadata.movement_title = header.value.clone();
            }
            "author" => score.metadata.composer = header.value.clone(),
            "transcriber" => score.metadata.poet = header.value.clone(),
            "book" => score.metadata.source = header.value.clone(),
            "office-part" => score.metadata.subtitle = header.value.clone(),
            "gabc-copyright" | "score-copyright" => {
                score.metadata.copyright = header.value.clone();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEI: &str = r#"<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1">
      <meiHead><fileDesc><titleStmt><title>Veni</title></titleStmt><pubStmt/>
        <notesStmt><annot type="gabc-header" n="name">Veni</annot>
        <annot type="gabc-header" n="mode">2</annot></notesStmt></fileDesc></meiHead>
      <music><body><mdiv><score>
        <scoreDef><staffGrp><staffDef n="1" lines="4">
          <clef shape="C" line="3"/></staffDef></staffGrp></scoreDef>
        <section><staff n="1"><layer n="1">
          <annot type="gabc-clef" n="0">c3</annot>
          <syllable><syl>Ve&lt;sp&gt;R/&lt;/sp&gt;ni</syl>
            <annot type="gabc-rubric" n="0">&lt;sp&gt;R/&lt;/sp&gt;</annot>
            <annot type="gabc-space" n="0">1</annot>
            <neume><nc pname="b" oct="3"><quilisma/></nc></neume>
            <annot type="gabc-note" n="0">gw.</annot>
            <annot type="gabc-note-sign Quilisma" n="0:0">w</annot>
            <annot type="gabc-note-sign PunctumMora" n="0:1">.</annot>
            <divLine form="minima"/><annot type="gabc-bar" n="0">,</annot>
          </syllable>
        </layer></staff></section>
      </score></mdiv></body></music></mei>"#;

    #[test]
    fn rebuilds_the_exact_gregorian_payload() {
        let root = scoretk_xml_common::parse(MEI.as_bytes()).expect("parse MEI neumes");
        let score = read(&root).expect("read neumes");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload is present");
        };
        assert_eq!(gregorian.clefs, vec!["c3"]);
        assert_eq!(gregorian.syllables.len(), 1);
        let syllable = &gregorian.syllables[0];
        assert_eq!(syllable.text, "Ve<sp>R/</sp>ni");
        assert_eq!(syllable.rubric.as_deref(), Some("<sp>R/</sp>"));
        assert!(syllable.space_after);
        assert_eq!(syllable.notes.len(), 1);
        assert_eq!(syllable.notes[0].letter, 'g');
        assert_eq!(syllable.notes[0].glyph, "w.");
        assert_eq!(syllable.notes[0].signs.len(), 2);
        assert_eq!(syllable.notes[0].signs[0].raw, "w");
        assert_eq!(syllable.notes[0].signs[1].kind, SignKind::PunctumMora);
        assert_eq!(syllable.bars, vec![BarKind::DivisioMinima]);
        assert!(gregorian.derived_modern_view);
        assert_eq!(score.notes.len(), 1, "the derived modern view is rebuilt");
    }

    #[test]
    fn reads_standard_mei_without_the_private_tokens() {
        let stripped = MEI
            .replace(
                r#"            <annot type="gabc-note" n="0">gw.</annot>
            <annot type="gabc-note-sign Quilisma" n="0:0">w</annot>
            <annot type="gabc-note-sign PunctumMora" n="0:1">.</annot>
"#,
                "",
            )
            .replace(r#"<annot type="gabc-bar" n="0">,</annot>"#, "");
        let root = scoretk_xml_common::parse(stripped.as_bytes()).expect("parse MEI neumes");
        let score = read(&root).expect("read neumes");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload is present");
        };
        let note = &gregorian.syllables[0].notes[0];
        assert_eq!(note.letter, 'g');
        assert_eq!(note.glyph, "w");
        assert_eq!(note.signs.len(), 1);
        assert_eq!(note.signs[0].kind, SignKind::Quilisma);
    }
}
