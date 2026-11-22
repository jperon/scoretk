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

use super::spanners::{SpanFamily, SpanKey, SpannerLink};
use super::xml::{Content, Node};
use super::NoteKey;
use crate::read::notation_names;

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
pub(super) fn adapt(root: &mut Node, ties: &mut Vec<super::TieLink>, links: &mut Vec<SpannerLink>) {
    let Some(score) = root.child_mut("Score") else {
        return;
    };
    adapt_score(score, ties, links);
}

fn adapt_score(score: &mut Node, ties: &mut Vec<super::TieLink>, links: &mut Vec<SpannerLink>) {
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
                adapt_staff(child, staff, ties, links);
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
fn adapt_staff(
    staff: &mut Node,
    staff_index: usize,
    ties: &mut Vec<super::TieLink>,
    links: &mut Vec<SpannerLink>,
) {
    // Seuls les `<Measure>` deviennent des mesures du modèle : les cadres
    // `<VBox>`/`<HBox>` ne comptent pas dans l'index utilisé pour retrouver les
    // notes des liaisons.
    let mut index = 0usize;
    for child in staff.children_mut() {
        if child.name == "Measure" {
            adapt_measure(child, index, staff_index, ties, links);
            index += 1;
        }
    }
}

/// Regroupe les éléments d'une mesure 2.x en voix, reshape les n-olets et
/// retire les liaisons idées, à la manière de `readMeasure206`.
fn adapt_measure(
    measure: &mut Node,
    index: usize,
    staff: usize,
    ties: &mut Vec<super::TieLink>,
    links: &mut Vec<SpannerLink>,
) {
    let old = std::mem::take(&mut measure.content);
    let mut groups: [Vec<Node>; 4] = std::array::from_fn(|_| Vec::new());
    let mut chord_ordinals = [0usize; 4];
    let mut kept = Vec::new();
    // Ordinal des notes par voix dans la mesure : `find_note` retrouve ensuite
    // les notes par leur position globale (mesure, piste, rang). Le compteur
    // doit donc survivre d'un accord à l'autre, sinon toutes les premières
    // notes d'accord reçoivent le rang 0 et les liaisons intra-mesure se
    // retrouvent sur la même note.
    let mut ordinals = [0usize; 4];

    for item in old {
        let Content::Element(mut node) = item else {
            continue;
        };
        let voice = node
            .child("track")
            .and_then(Node::int_text)
            .map_or(0, |track| (track.rem_euclid(4)) as usize)
            .min(3);
        // `readMeasure206` (read206.cpp:2834-2873) : une volta 2.x est définie
        // par `<Volta id>` et refermée par `<endSpanner id>`. On collecte les
        // deux moitiés ; `fix_spanners` les apparie après la lecture 3.x.
        match node.name.as_str() {
            "Volta" => {
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Volta);
                    link.start = Some(measure_key(index, staff));
                    link.endings = node
                        .child("endings")
                        .map(|node| parse_endings(&node.trimmed_text()))
                        .unwrap_or_default();
                    link.closed = node
                        .child("endHook")
                        .and_then(Node::int_text)
                        .is_some_and(|value| value != 0);
                    link.text = node
                        .child("beginText")
                        .and_then(|node| node.child("text"))
                        .map(Node::text);
                    links.push(link);
                }
            }
            "Ottava" => {
                // `readOttava` (read206.cpp:2286) : `"8va"`, `"8vb"`,
                // `"15ma"`, `"15mb"` (22ma/22mb n'ont pas d'équivalent dans le
                // modèle et retombent sur l'octave correspondante).
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Ottava);
                    // `readMeasure206` place l'ottava au tick courant, qui peut
                    // être au milieu de la mesure si des accords l'ont déjà
                    // précédée : on s'ancre sur le prochain accord.
                    link.start = Some(if chord_ordinals[voice] == 0 {
                        measure_key(index, staff)
                    } else {
                        SpanKey::Chord {
                            measure: index,
                            track: ((staff * 4) + voice) as u32,
                            ordinal: chord_ordinals[voice],
                        }
                    });
                    link.ottava_type = Some(
                        match node.child("subtype").map(Node::trimmed_text).as_deref() {
                            Some("8vb") => scoretk_core::model::OttavaType::OctaveDown,
                            Some("15ma") => scoretk_core::model::OttavaType::FifteenthUp,
                            Some("15mb") => scoretk_core::model::OttavaType::FifteenthDown,
                            Some("22mb") => scoretk_core::model::OttavaType::FifteenthDown,
                            _ => scoretk_core::model::OttavaType::OctaveUp,
                        },
                    );
                    links.push(link);
                }
            }
            "HairPin" => {
                // `readHairpin206` (read206.cpp:2318) : `<subtype>` entier,
                // 1/3 = diminuendo, >= 2 = trait pointillé.
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Hairpin);
                    link.start = Some(measure_key(index, staff));
                    let subtype = node.child("subtype").and_then(Node::int_text).unwrap_or(0);
                    link.hairpin_kind = Some(if subtype == 1 || subtype == 3 {
                        scoretk_core::model::HairpinType::Diminuendo
                    } else {
                        scoretk_core::model::HairpinType::Crescendo
                    });
                    link.hairpin_line = subtype >= 2;
                    link.hairpin_niente = node
                        .child("hairpinCircledTip")
                        .and_then(Node::int_text)
                        .is_some_and(|value| value != 0);
                    link.hairpin_direction = node.child("direction").map(Node::trimmed_text);
                    link.text = node
                        .child("beginText")
                        .and_then(|node| node.child("text"))
                        .map(Node::text);
                    links.push(link);
                }
            }
            "Slur" => {
                // `readSlur206` (read206.cpp:2598) enregistre la définition par
                // id ; l'ancrage définitif viendra du `<Slur type="start">`
                // porté par l'accord.
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Slur);
                    let mut key = measure_key(index, staff);
                    if let Some(track) = node.child("track").and_then(Node::int_text) {
                        if let SpanKey::Measure { track: value, .. } = &mut key {
                            *value = track as u32;
                        }
                    }
                    link.start = Some(key);
                    link.slur_up = node.child("up").map(Node::trimmed_text);
                    links.push(link);
                }
            }
            "Trill" => {
                // `Read206::readTrill206` (`read206.cpp:2307-2330`) puis
                // `readMeasure206` (`:2818-2837`) : `<Trill id>` est posé à
                // `ctx.tick()`, c'est-à-dire au tick courant, et refermé par un
                // `<endSpanner id>`. Un trille qui suit des accords dans la
                // mesure commence donc sur le prochain accord, pas au début de
                // la mesure.
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Trill);
                    link.start = Some(if chord_ordinals[voice] == 0 {
                        measure_key(index, staff)
                    } else {
                        SpanKey::Chord {
                            measure: index,
                            track: ((staff * 4) + voice) as u32,
                            ordinal: chord_ordinals[voice],
                        }
                    });
                    link.trill_type = node
                        .child("subtype")
                        .map(|child| notation_names::trill_type(&child.trimmed_text()));
                    links.push(link);
                }
            }
            "Glissando" => {
                // `readMeasure206` (`read206.cpp:2871-2892`) : même couplage
                // `<Glissando id>` / `<endSpanner id>` que pour les trilles.
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Glissando);
                    link.start = Some(measure_key(index, staff));
                    let subtype = node.child("subtype").map(Node::trimmed_text);
                    let style = node
                        .child("lineStyle")
                        .map(|child| crate::read::notation_names::line_style(&child.trimmed_text()))
                        .unwrap_or(scoretk_core::model::LineStyle::Solid);
                    link.glissando = Some(scoretk_core::model::Glissando::new(
                        notation_names::glissando_type(subtype.as_deref(), style),
                    ));
                    link.text = node.child("text").map(Node::text);
                    links.push(link);
                }
            }
            "endSpanner" => {
                if let Some(id) = node.attr_i32("id") {
                    let mut link = SpannerLink::new(id, SpanFamily::Volta);
                    // `readMeasure206` (`read206.cpp:2834-2852`) ferme le
                    // spanner à `ctx.tick()`, c'est-à-dire au tick courant : un
                    // `<endSpanner>` mesure-level peut tomber entre deux
                    // accords. L'ancrer sur le début de mesure raccourcit
                    // l'ottava d'un accord entier (§19.8 de mscx-format.md).
                    link.end = Some(if chord_ordinals[voice] == 0 {
                        measure_key(index, staff)
                    } else {
                        SpanKey::Chord {
                            measure: index,
                            track: ((staff * 4) + voice) as u32,
                            ordinal: chord_ordinals[voice],
                        }
                    });
                    links.push(link);
                }
            }
            _ => {}
        }
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
        // Deux comptages par voix dans la mesure : les accords/silences
        // (extrémités de spanners) et les notes (extrémités de liaisons).
        let chord_ordinal = if node.name == "Chord" || node.name == "Rest" {
            let value = chord_ordinals[voice];
            chord_ordinals[voice] += 1;
            value
        } else {
            0
        };
        adapt_element(
            &mut node,
            &mut AdaptCtx {
                measure: index,
                staff,
                voice,
                chord_ordinal,
                ties,
                links,
                ordinal: &mut ordinals[voice],
            },
        );
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

/// Clé d'ancrage d'un élément au niveau mesure (`read206.cpp:2874-2875`).
fn measure_key(measure: usize, staff: usize) -> SpanKey {
    SpanKey::Measure {
        measure,
        track: (staff * 4) as u32,
    }
}

/// `<endings>` d'une volta 2.x : liste d'entiers séparés par des virgules
/// (`readVolta206`, `read206.cpp:2216-2222`).
fn parse_endings(text: &str) -> Vec<u8> {
    text.split(',')
        .filter_map(|value| value.trim().parse::<u8>().ok())
        .collect()
}

/// Adapte un élément musical : textes système, symboles de tempo et liaisons.
/// Contexte d'adaptation d'un élément musical 2.x. Deux comptages par voix
/// coexistent : les accords/silences ancrent les spanners, les notes ancrent
/// les liaisons.
struct AdaptCtx<'a> {
    measure: usize,
    staff: usize,
    voice: usize,
    /// Rang de l'accord ou silence dans la voix (`SpanKey::Chord`).
    chord_ordinal: usize,
    /// Rang de la note dans la voix, maintenu d'un accord à l'autre.
    ordinal: &'a mut usize,
    ties: &'a mut Vec<super::TieLink>,
    links: &'a mut Vec<SpannerLink>,
}

fn adapt_element(node: &mut Node, ctx: &mut AdaptCtx<'_>) {
    // `readChordRestProperties206` (`read206.cpp:1821-1822`) : le champ 2.x
    // `<move>` est l'ancien nom de `staffMove`, que `read302` sait lire. Sans
    // ce port, les notes entre portées des partitions 2.x perdent leur
    // `staffMove` et l'export MusicXML écrit `<staff>1</staff>` au lieu de
    // `<staff>2</staff>` (`exportmusicxml.cpp:4469`).
    if matches!(node.name.as_str(), "Chord" | "Rest") {
        for child in node.children_mut() {
            if child.name == "move" {
                child.name = "staffMove".to_owned();
            }
        }
    }
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
            let chord_key = SpanKey::Chord {
                measure: ctx.measure,
                track: ((ctx.staff * 4) + ctx.voice) as u32,
                ordinal: ctx.chord_ordinal,
            };
            // `readChordRestProperties206` (read206.cpp:1823) : les moitiés de
            // slur 2.x sont portées par l'accord avec `type="start"`/`"stop"`.
            for child in node.children() {
                if child.name != "Slur" {
                    continue;
                }
                let Some(id) = child.attr_i32("id") else {
                    continue;
                };
                let mut link = SpannerLink::new(id, SpanFamily::Slur);
                if child.attr("type") == Some("stop") {
                    link.end = Some(chord_key);
                } else {
                    link.start = Some(chord_key);
                }
                link.slur_up = child.child("up").map(Node::trimmed_text);
                ctx.links.push(link);
            }
            // Les notes d'un accord d'agrément ne sont pas des `ChordRest`
            // du modèle : elles vivent dans `Chord::grace_chords`, que
            // `find_note` ne parcourt pas. Les compter ici décalait tous les
            // rangs de la voix et empêchait `fix_ties` de retrouver la note de
            // départ (les 14 liaisons de ZORBA après une acciaccatura).
            let grace = [
                "appoggiatura",
                "acciaccatura",
                "grace4",
                "grace8after",
                "grace16",
                "grace16after",
                "grace32",
                "grace32after",
            ]
            .iter()
            .any(|tag| node.child(tag).is_some());
            for child in node.children_mut() {
                if child.name != "Note" {
                    continue;
                }
                let key = NoteKey {
                    measure: ctx.measure,
                    track: ((ctx.staff * 4) + ctx.voice) as u32,
                    ordinal: *ctx.ordinal,
                };
                if !grace {
                    *ctx.ordinal += 1;
                }
                if let Some(tie) = child.child("Tie") {
                    if let Some(id) = tie.attr_i32("id") {
                        ctx.ties.push(super::TieLink {
                            id,
                            start: Some(key),
                            end: None,
                        });
                    }
                    child.retain_without("Tie");
                }
                // `readNoteProperties206` (read206.cpp:1221) : un glissando
                // 2.x peut être porté par une note, pas seulement par une
                // mesure ; c'est le cas du corpus (mscz-8).
                let glissando = child.child("Glissando").map(|gliss| {
                    let id = gliss.attr_i32("id");
                    let subtype = gliss.child("subtype").map(Node::trimmed_text);
                    let style = gliss
                        .child("lineStyle")
                        .map(|child| crate::read::notation_names::line_style(&child.trimmed_text()))
                        .unwrap_or(scoretk_core::model::LineStyle::Solid);
                    let text = gliss.child("text").map(Node::text);
                    (id, subtype, style, text)
                });
                if let Some((Some(id), subtype, style, text)) = glissando {
                    let mut link = SpannerLink::new(id, SpanFamily::Glissando);
                    link.start = Some(SpanKey::Chord {
                        measure: ctx.measure,
                        track: ((ctx.staff * 4) + ctx.voice) as u32,
                        ordinal: ctx.chord_ordinal,
                    });
                    link.glissando = Some(scoretk_core::model::Glissando::new(
                        notation_names::glissando_type(subtype.as_deref(), style),
                    ));
                    link.text = text;
                    ctx.links.push(link);
                    child.retain_without("Glissando");
                }
                if let Some(end) = child.child("endSpanner") {
                    if let Some(id) = end.attr_i32("id") {
                        ctx.ties.push(super::TieLink {
                            id,
                            start: None,
                            end: Some(key),
                        });
                        // Un soufflet peut aussi se refermer sur une note
                        // (`readNoteProperties206`, read206.cpp:1221).
                        let mut link = SpannerLink::new(id, SpanFamily::Volta);
                        link.end = Some(SpanKey::Chord {
                            measure: ctx.measure,
                            track: ((ctx.staff * 4) + ctx.voice) as u32,
                            ordinal: ctx.chord_ordinal,
                        });
                        ctx.links.push(link);
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
