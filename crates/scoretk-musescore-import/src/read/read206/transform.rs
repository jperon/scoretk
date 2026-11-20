// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read206/read206.cpp` de MuseScore Studio (les
// fonctions `Read206::readScoreTag`, `readStaffContent206`, `readMeasure206`,
// `readChord`, `readNote206` et `readTuplet206`), Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Adaptation structurelle d'un arbre 2.x à la forme lue par `read302`.
//!
//! `read206.cpp` délègue la lecture de presque tous les éléments au lecteur
//! commun `read400::TRead` (déjà porté dans `read302`). Seules différent les
//! conventions d'ensemble du format 2.x : les voix ne sont pas des blocs
//! `<voice>` mais des éléments portant un `<track>` et un `<tick>` absolu, les
//! n-olets sont définis par `<Tuplet id>` et référencés depuis les accords, et
//! les liaisons utilisent `<Tie id>` / `<endSpanner id>`. On réécrit ici ces
//! conventions, puis on confie l'arbre obtenu à `read302` : c'est le même
//! partage du travail que dans le C++ amont.

use super::xml::{Content, Node};
use super::NoteKey;

/// Balises qui restent des enfants directs de `<Measure>` en 3.x.
///
/// Toutes les autres balises musicales entrent dans un bloc `<voice>`: le
/// lecteur 3.x (`MeasureRead::readVoice`) les attend à cet endroit, tandis que
/// `read206::readMeasure206` les lisait directement sous la mesure.
const MEASURE_LEVEL: &[&str] = &[
    "LayoutBreak",
    "startRepeat",
    "endRepeat",
    "Marker",
    "Jump",
    "stretch",
    "noOffset",
    "measureNumberMode",
    "irregular",
    "multiMeasureRest",
    "breakMultiMeasureRest",
    "sysInitBarLineType",
    "vspacer",
    "vspacerUp",
    "vspacerDown",
    "visible",
    "slashStyle",
    "MeasureNumber",
    "SystemDivider",
];

/// `<Text>` 2.x dont le style doit devenir un `SystemText` 3.x.
///
/// `read206.cpp:3023` classe ces textes comme système ; MuseScore 3 a séparé
/// les deux types.
const SYSTEM_STYLES: &[&str] = &["System", "Tempo", "Marker", "Jump", "Volta"];

/// Adapte l'arbre complet et collecte les liaisons à reconnecter sur le modèle.
pub(super) fn adapt(root: &mut Node, ties: &mut Vec<super::TieLink>) {
    let Some(score) = root.child_mut("Score") else {
        return;
    };
    adapt_score(score, ties);
}

fn adapt_score(score: &mut Node, ties: &mut Vec<super::TieLink>) {
    // `readStaff` 2.x porte `<transposeDiatonic>`/`<transposeChromatic>` sur la
    // portée, alors que le lecteur 3.x les lit dans `<Instrument>`. On les
    // remonte dans l'instrument de la partie qui déclare cette portée.
    let transpositions: Vec<(usize, Option<String>, Option<String>)> = score
        .children()
        .filter(|child| child.name == "Staff")
        .map(|staff| {
            let index = staff
                .attr_i32("id")
                .map_or(usize::MAX, |id| (id.max(1) - 1) as usize);
            (
                index,
                staff.child("transposeDiatonic").map(Node::trimmed_text),
                staff.child("transposeChromatic").map(Node::trimmed_text),
            )
        })
        .collect();

    let mut index = 0usize;
    for child in score.children_mut() {
        match child.name.as_str() {
            "Staff" => {
                child.retain_without("transposeDiatonic");
                child.retain_without("transposeChromatic");
                // `StaffRead::readStaff` numérote les portées par l'attribut
                // `id` (1-based), pas par la position du bloc.
                let staff = child
                    .attr_i32("id")
                    .map_or(index, |id| (id.max(1) - 1) as usize);
                adapt_staff(child, staff, ties);
                index += 1;
            }
            "Part" => {
                let staff_indices: Vec<usize> = child
                    .children()
                    .filter(|staff| staff.name == "Staff")
                    .filter_map(|staff| staff.attr_i32("id"))
                    .map(|id| (id.max(1) - 1) as usize)
                    .collect();
                if let Some(instrument) = child.child_mut("Instrument") {
                    for staff_index in staff_indices {
                        if let Some((_, Some(diatonic), Some(chromatic))) = transpositions
                            .iter()
                            .find(|(index, _, _)| *index == staff_index)
                        {
                            insert_text_child(instrument, "transposeDiatonic", diatonic);
                            insert_text_child(instrument, "transposeChromatic", chromatic);
                        }
                    }
                }
            }
            "siglist" => {
                // `Read206::readTimeSigMap` : la carte des métriques ne sert
                // qu'à l'obsolète `<ticklen>` ; les `<TimeSig>` des mesures
                // suffisent au lecteur 3.x.
                child.name = "_siglist".to_owned();
            }
            _ => {}
        }
    }
}

/// Insère (ou remplace) un enfant texte dans un élément.
fn insert_text_child(parent: &mut Node, name: &str, value: &str) {
    parent.retain_without(name);
    let mut node = Node::new(name);
    node.content.push(Content::Text(value.to_owned()));
    parent.push(node);
}

/// Adapte le contenu d'une `<Staff id="…">` (numéro de portée 0-basé `staff`).
fn adapt_staff(staff: &mut Node, staff_index: usize, ties: &mut Vec<super::TieLink>) {
    // Seuls les `<Measure>` deviennent des mesures du modèle : les cadres
    // `<VBox>`/`<HBox>` ne comptent pas dans l'index utilisé pour retrouver les
    // notes des liaisons.
    let mut index = 0usize;
    for child in staff.children_mut() {
        if child.name == "Measure" {
            adapt_measure(child, index, staff_index, ties);
            index += 1;
        }
    }
}

/// Regroupe les éléments d'une mesure 2.x en voix, reshape les n-olets et
/// retire les liaisons idées, à la manière de `readMeasure206`.
fn adapt_measure(measure: &mut Node, index: usize, staff: usize, ties: &mut Vec<super::TieLink>) {
    let old = std::mem::take(&mut measure.content);
    let mut groups: [Vec<Node>; 4] = std::array::from_fn(|_| Vec::new());
    let mut kept = Vec::new();

    for item in old {
        let Content::Element(mut node) = item else {
            continue;
        };
        if MEASURE_LEVEL.contains(&node.name.as_str()) {
            kept.push(Content::Element(node));
            continue;
        }
        // Un `<tick>` mesure-level positionne toutes les voix dans le format
        // 2.x ; on le répète dans chaque groupe pour reproduire ce comportement.
        if node.name == "tick" {
            for group in &mut groups {
                group.push(node.clone());
            }
            continue;
        }
        let voice = node
            .child("track")
            .and_then(Node::int_text)
            .map_or(0, |track| (track.rem_euclid(4)) as usize)
            .min(3);
        adapt_element(&mut node, index, staff, voice, ties);
        groups[voice].push(node);
    }

    for group in groups {
        if group.is_empty() {
            continue;
        }
        let mut block = Node::new("voice");
        block.content = group.into_iter().map(Content::Element).collect();
        reshape_tuplets(&mut block);
        kept.push(Content::Element(block));
    }
    measure.content = kept;
}

/// Adapte un élément musical : textes système, symboles de tempo et liaisons.
fn adapt_element(
    node: &mut Node,
    measure: usize,
    staff: usize,
    voice: usize,
    ties: &mut Vec<super::TieLink>,
) {
    match node.name.as_str() {
        "Text" | "StaffText" => {
            let system = node
                .child("style")
                .is_some_and(|style| SYSTEM_STYLES.contains(&style.trimmed_text().as_str()));
            if system {
                node.name = "SystemText".to_owned();
            }
        }
        "Tempo" => {
            // `readTempoText` (read206.cpp:1548) : `<sym>unicode` → `<sym>met`.
            replace_in_texts(node, "<sym>unicode", "<sym>met");
        }
        "Marker" => {
            // `convertMScoreFontMarkerSymbols206` (read206.cpp:1500).
            replace_in_texts(node, "\u{e177}", "<sym>segno</sym>");
            replace_in_texts(node, "\u{e178}", "<sym>coda</sym>");
            replace_in_texts(node, "\u{e179}", "<sym>codaSquare</sym>");
        }
        "Chord" => {
            let mut ordinal = 0usize;
            for child in node.children_mut() {
                if child.name != "Note" {
                    continue;
                }
                let key = NoteKey {
                    measure,
                    track: ((staff * 4) + voice) as u32,
                    ordinal,
                };
                ordinal += 1;
                if let Some(tie) = child.child("Tie") {
                    if let Some(id) = tie.attr_i32("id") {
                        ties.push(super::TieLink {
                            id,
                            start: Some(key),
                            end: None,
                        });
                    }
                    child.retain_without("Tie");
                }
                if let Some(end) = child.child("endSpanner") {
                    if let Some(id) = end.attr_i32("id") {
                        ties.push(super::TieLink {
                            id,
                            start: None,
                            end: Some(key),
                        });
                    }
                    child.retain_without("endSpanner");
                }
            }
        }
        _ => {}
    }
}

/// Remplace une sous-chaîne dans le balisage des `<text>` descendants.
fn replace_in_texts(node: &mut Node, from: &str, to: &str) {
    for item in &mut node.content {
        match item {
            Content::Text(text) => {
                if text.contains(from) {
                    *text = text.replace(from, to);
                }
            }
            Content::Element(child) => replace_in_texts(child, from, to),
        }
    }
}

/// Transforme les `<Tuplet id>` 2.x en séquence `<Tuplet> … <endTuplet/>`
/// comprise par le lecteur 3.x.
///
/// Dans `read206.cpp`, `<Tuplet id="N">` crée le n-olet et les accords portant
/// `<Tuplet>N</Tuplet>` s'y rattachent (`readDurationProperties206`). Le
/// lecteur 3.x garde le n-olet courant ouvert jusqu'à `<endTuplet/>`.
fn reshape_tuplets(voice: &mut Node) {
    let old = std::mem::take(&mut voice.content);
    let is_chord_rest = |node: &Node| node.name == "Chord" || node.name == "Rest";
    let referenced_id =
        |node: &Node| -> Option<i32> { node.child("Tuplet")?.trimmed_text().parse::<i32>().ok() };

    // Dernier accord qui référence chaque n-olet : c'est là que le lecteur 3.x
    // doit le fermer. Un n-olet laissé ouvert appliquerait son ratio aux accords
    // suivants et en ferait disparaître certains par collision de tick.
    let definitions: Vec<(usize, i32)> = old
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            Content::Element(node) if node.name == "Tuplet" => {
                node.attr_i32("id").map(|id| (index, id))
            }
            _ => None,
        })
        .collect();
    let mut closes: std::collections::BTreeMap<usize, Vec<i32>> = std::collections::BTreeMap::new();
    for (definition_index, id) in definitions {
        let last = old
            .iter()
            .enumerate()
            .filter(|(_, item)| match item {
                Content::Element(node) => is_chord_rest(node) && referenced_id(node) == Some(id),
                _ => false,
            })
            .map(|(index, _)| index)
            .max()
            .unwrap_or(definition_index);
        closes.entry(last).or_default().push(id);
    }

    let mut result = Vec::new();
    for (index, item) in old.into_iter().enumerate() {
        if let Content::Element(mut node) = item {
            if node.name == "Tuplet" {
                node.attrs.retain(|(key, _)| key != "id");
            } else if is_chord_rest(&node) {
                node.retain_without("Tuplet");
            }
            result.push(Content::Element(node));
        } else {
            result.push(item);
        }
        if let Some(ids) = closes.get(&index) {
            for _ in ids {
                result.push(Content::Element(Node::new("endTuplet")));
            }
        }
    }
    voice.content = result;
}
