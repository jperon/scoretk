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
//! `Gregorian::build_derived_score`. Les éléments MEI 5.1 standard suffisent :
//! les glyphes sont reconstruits depuis les enfants normatifs de `<nc>`
//! (§6.4.1, §6.4.8), les en-têtes depuis `<meiHead>` (ch. 3) et les barres
//! depuis `<divLine @form>` (§6.4.1). Les annotations `gabc-*` ne servent
//! qu'à relire les anciens fichiers produits avec l'extension privée.

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
    // Le layout GABC (commentaires, lignes vides, `;;`, préfixe) traverse MEI
    // par `@label` sur `<meiHead>`, comme les séparateurs exacts en §19.
    let head_label = find_first(root, "meiHead").and_then(|head| head.attribute("label"));
    scoretk_mei_common::gabc_layout::read_header_label(head_label, &mut gregorian);
    read_headers(root, &mut gregorian);
    read_staff_lines(root, &mut gregorian);

    let Some(layer) = find_first(root, "layer") else {
        return Err(Error::Unsupported("couche neumatique absente".to_owned()));
    };
    initial_clefs(root, layer, &mut gregorian);
    // `@label` est le porteur MEI standard de l'espacement exact du source ;
    // sans lui (encodage tiers) le payload reste sans séparateur connu.
    gregorian.opening_separator = neumes::layout_from_label(layer.attribute("label"), "open");
    let mut current_clef = gregorian
        .clefs
        .first()
        .cloned()
        .unwrap_or_else(|| "c4".to_owned());

    for syllable_node in layer.children.iter().filter(|node| node.name == "syllable") {
        let mut syllable = Syllable::default();
        let syl = syllable_node.child("syl");
        if let Some(syl) = syl {
            // Le texte exact est conservé (espaces de bord compris) :
            // `text_trimmed()` supprimerait les espaces de mot de GABC.
            syllable.text = neumes::syl_text(syl);
            let con = syl.attribute("con").unwrap_or("");
            let wordpos = syl.attribute("wordpos").unwrap_or("");
            syllable.space_after = con == "s" || wordpos == "t";
        }
        syllable.rubric = rubric(&syllable.text);
        for child in &syllable_node.children {
            if child.name == "signifLet" && child.attribute("type") == Some("gabc-above") {
                syllable.above_text = Some(child.text.clone());
            }
        }

        // Anciennes annotations : uniquement si le porteur standard manque.
        for annot in syllable_node
            .children
            .iter()
            .filter(|node| node.name == "annot")
        {
            let (kind, _) = annot_type(annot);
            match kind {
                neumes::ABOVE_ANNOT if syllable.above_text.is_none() => {
                    syllable.above_text = Some(annot.text.clone());
                }
                neumes::RUBRIC_ANNOT if syllable.rubric.is_none() => {
                    syllable.rubric = Some(annot.text.clone());
                }
                neumes::SPACE_ANNOT => {
                    syllable.space_after = annot.text.trim() != "0" && !annot.text.is_empty();
                }
                _ => {}
            }
        }
        // Le porteur exact est `@label` (`att.labelled`, attribut standard).
        // Sans lui — MEI tiers ou ancien fichier — le séparateur est déduit
        // du marqueur de fin de mot, comme l'écrivain GABC historique le
        // réémettait : un espace après les syllabes `wordpos="t"`, rien
        // sinon. Le champ reste présent pour que l'écrivain GABC n'invente
        // pas un saut de ligne là où le MEI n'en porte pas.
        syllable.separator_after =
            neumes::layout_from_label(syl.and_then(|node| node.attribute("label")), "sep").or_else(
                || {
                    Some(if syllable.space_after {
                        " ".to_owned()
                    } else {
                        String::new()
                    })
                },
            );

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
                if note.signs.is_empty() {
                    note.signs = nc_signs(nc, index, &sign_anotts);
                }
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
                    Some("v") => {
                        if node.attribute("place") == Some("below") {
                            "'0"
                        } else {
                            "'"
                        }
                    }
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
    // La seconde portée manuscrite (NABC) est parallèle à la portée carrée :
    // ses snippets sont attachés aux syllabes correspondantes, d'abord par
    // `@corresp`, sinon par position (`mei.md` §13).
    if super::nabc::present(root) {
        super::nabc::read_into(root, &mut gregorian);
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

/// En-têtes GABC reconstruits depuis les éléments standards de `<meiHead>`
/// (*Guidelines* ch. 3) : `@n` est l'index source, `@role`/`@type` le nom du
/// champ. Les `<annot type="gabc-header">` ne sont lus qu'en leur absence.
fn read_headers(root: &Element, gregorian: &mut Gregorian) {
    let mut found: Vec<(usize, GregorianHeader)> = Vec::new();
    let mut nodes = Vec::new();
    collect(root, "title", &mut nodes);
    collect(root, "term", &mut nodes);
    collect(root, "persName", &mut nodes);
    collect(root, "rend", &mut nodes);
    for node in nodes {
        let Some(index) = node
            .attribute("n")
            .and_then(|value| value.parse::<usize>().ok())
        else {
            continue;
        };
        let name = match node.name.as_str() {
            "title" => node.attribute("type").unwrap_or("name").to_owned(),
            "term" | "rend" => node.attribute("type").unwrap_or("").to_owned(),
            "persName" => node.attribute("role").unwrap_or("").to_owned(),
            _ => continue,
        };
        if !name.is_empty() {
            found.push((
                index,
                GregorianHeader {
                    name,
                    value: node.text.clone(),
                },
            ));
        }
    }
    found.sort_by_key(|(index, _)| *index);
    gregorian.headers = found.into_iter().map(|(_, header)| header).collect();

    if gregorian.headers.is_empty() {
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
}

fn rubric(text: &str) -> Option<String> {
    for tag in ["sp", "alt"] {
        let open = format!("<{tag}>");
        if let Some(start) = text.find(&open) {
            let close = format!("</{tag}>");
            let end = text[start..]
                .find(&close)
                .map(|offset| start + offset + close.len())
                .unwrap_or(text.len());
            return Some(text[start..end].to_owned());
        }
    }
    None
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
    let mut token = neumes::clef_token(
        clef.attribute("shape").unwrap_or("C"),
        clef.attribute("line")
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(3),
    );
    if clef.attribute("type") == Some("clef-flat") {
        token.insert(1, 'b');
    }
    token
}

/// Note GABC reconstruite depuis `<nc>` et ses modifiers MEI 5.1 standard.
fn nc_note(nc: &Element, clef: &str, private: Option<&&Element>) -> GregorianNote {
    let letter = if let Some(annot) = private {
        let mut chars = annot.text.chars();
        let letter = chars.next();
        // Ancienne annotation : le glyphe y est encore copié, mais les
        // éléments standard restent prioritaires quand ils sont présents.
        let from_elements = element_glyph(nc);
        if let Some(letter) = letter {
            return GregorianNote {
                letter,
                glyph: if from_elements.is_empty() {
                    chars.collect()
                } else {
                    from_elements
                },
                ..GregorianNote::default()
            };
        }
        letter
    } else {
        None
    };
    let mut letter = letter
        .or_else(|| neumes::nc_letter(nc.attribute("pname"), parse_oct(nc), clef))
        .unwrap_or('g');
    // Une lettre majuscule GABC est un punctum inclinatum (`Gabc.tex:454-468`),
    // encodé par `@type` (et `@tilt="se"`, *Guidelines* §6.4.2).
    if nc.attribute("type") == Some("punctum-inclinatum") {
        letter = letter.to_ascii_uppercase();
    }
    GregorianNote {
        letter,
        glyph: element_glyph(nc),
        ..GregorianNote::default()
    }
}

/// Concatène les jetons GABC déduits des enfants normatifs de `<nc>`.
fn element_glyph(nc: &Element) -> String {
    let mut glyph = String::new();
    for child in &nc.children {
        if let Some(part) = neumes::part_from_mei(child) {
            glyph.push_str(&part.raw);
        }
    }
    if glyph.is_empty() {
        // Neume tiers sans glyphe GABC : les modificateurs standards donnent
        // tout de même une lecture musicale correcte.
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
    }
    glyph
}

fn parse_oct(nc: &Element) -> Option<u8> {
    nc.attribute("oct")?.parse().ok()
}

/// Signes d'une note : éléments MEI 5.1 standard, ou annotations pour les
/// anciens fichiers.
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
        if let Some(part) = neumes::part_from_mei(child) {
            if let Some(kind) = part.kind {
                signs.push(NeumeSign::new(kind, part.raw));
            }
        }
    }
    signs
}

fn episema_kind(raw: &str) -> SignKind {
    if raw.starts_with('\'') {
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
      <meiHead><fileDesc><titleStmt><title n="0">Veni</title></titleStmt><pubStmt/>
        <classification><termList><term type="mode" n="1">2</term></termList></classification>
        </fileDesc></meiHead>
      <music><body><mdiv><score>
        <scoreDef><staffGrp><staffDef n="1" lines="4">
          <clef shape="C" line="3"/></staffDef></staffGrp></scoreDef>
        <section><staff n="1"><layer n="1">
          <syllable><syl wordpos="t" con="s">Ve&lt;sp&gt;R/&lt;/sp&gt;ni</syl>
            <neume n="0" type="punctum"><nc pname="b" oct="3"><quilisma/><signifLet type="PunctumMora"/></nc></neume>
            <divLine form="minima"/>
          </syllable>
        </layer></staff></section>
      </score></mdiv></body></music></mei>"#;

    #[test]
    fn rebuilds_the_gregorian_payload_from_standard_carriers() {
        let root = scoretk_xml_common::parse(MEI.as_bytes()).expect("parse MEI neumes");
        let score = read(&root).expect("read neumes");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload is present");
        };
        assert_eq!(gregorian.clefs, vec!["c3"]);
        assert_eq!(gregorian.headers.len(), 2);
        assert_eq!(gregorian.headers[0].name, "name");
        assert_eq!(gregorian.headers[1].name, "mode");
        let syllable = &gregorian.syllables[0];
        assert_eq!(syllable.text, "Ve<sp>R/</sp>ni");
        assert_eq!(syllable.rubric.as_deref(), Some("<sp>R/</sp>"));
        assert!(syllable.space_after);
        assert_eq!(syllable.notes[0].letter, 'g');
        assert_eq!(syllable.notes[0].glyph, "w.");
        assert_eq!(syllable.notes[0].signs.len(), 2);
        assert_eq!(syllable.notes[0].signs[0].kind, SignKind::Quilisma);
        assert_eq!(syllable.notes[0].signs[1].kind, SignKind::PunctumMora);
        assert_eq!(syllable.bars, vec![BarKind::DivisioMinima]);
        assert!(gregorian.derived_modern_view);
        assert_eq!(score.notes.len(), 1, "the derived modern view is rebuilt");
    }

    #[test]
    fn body_layout_labels_are_restored_and_have_a_standard_fallback() {
        let labelled = MEI
            .replace("<layer n=\"1\">", "<layer n=\"1\" label=\"layout:open:L\">")
            .replace(
                "<syl wordpos=\"t\" con=\"s\">",
                "<syl wordpos=\"t\" con=\"s\" label=\"layout:sep:LL\">",
            );
        let root = scoretk_xml_common::parse(labelled.as_bytes()).expect("parse labelled MEI");
        let score = read(&root).expect("read neumes");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload");
        };
        assert_eq!(gregorian.opening_separator.as_deref(), Some("\n"));
        assert_eq!(
            gregorian.syllables[0].separator_after.as_deref(),
            Some("\n\n")
        );

        // Sans `@label` (MEI tiers), on retombe sur l'espacement historique :
        // un espace après une syllabe `wordpos="t"`, rien pour `wordpos="m"`.
        let root = scoretk_xml_common::parse(MEI.as_bytes()).expect("parse MEI neumes");
        let score = read(&root).expect("read neumes");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("L6 payload");
        };
        assert_eq!(gregorian.opening_separator, None);
        assert_eq!(gregorian.syllables[0].separator_after.as_deref(), Some(" "));
    }
}
