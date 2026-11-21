// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read400/tread.cpp` de MuseScore Studio, Copyright
// (C) MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Annotations de segment : nuances, textes, tempos, repères et accords.
//!
//! Portage partiel de `TRead::read(Dynamic*)`, `read(StaffText*)`,
//! `read(SystemText*)`, `read(TempoText*)`, `read(RehearsalMark*)` et
//! `read(Harmony*)`. Les éléments hors noyau sont ignorés par l'appelant.

use super::tables;
use super::xml::Node;
use super::Reader;
use scoretk_core::model::{
    text_style_defaults, tpc, Annotation, FiguredBass, FiguredBassContLine, FiguredBassItem,
    FiguredBassModifier, Harmony, InstrumentChange, Placement, RehearsalFrame, RehearsalMark,
    RichText, StaffText, SystemText, Tempo, TempoKind, TextFormat, TremoloBar, TremoloBarPoint,
};

/// Lit une annotation de segment, ou `None` si l'élément n'en est pas une.
///
/// `fermata_placement` est le placement par défaut d'un point d'orgue, déduit
/// de la piste par `MeasureRead::readVoice` ; il ne sert qu'à `<Fermata>`.
pub(crate) fn read(
    reader: &Reader,
    node: &Node,
    fermata_placement: Placement,
) -> Option<Annotation> {
    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()));
    // Le style fixe le placement et la police de départ ; les propriétés
    // `italic`/`bold`/`underline`/`strike` du texte les écrasent ensuite.
    let style_name = node
        .child("style")
        .map(|child| child.trimmed_text())
        .unwrap_or_default();
    let (style_placement, mut style_format) = text_style_defaults(&style_name);
    // Style « Expression » de MuseScore 3.x : italique 11 points.
    if style_name.eq_ignore_ascii_case("expression") {
        style_format.font_size = Some(11.0);
    }

    let annotation = match node.name() {
        "Dynamic" => {
            let dynamic = super::nuances::read_dynamic(node);
            Annotation::Dynamic(dynamic)
        }
        "StaffText" | "Text" => Annotation::StaffText(StaffText {
            text: rich_text(node, &style_format),
            placement: placement.unwrap_or(style_placement),
            visible: visible(node),
        }),
        "SystemText" => Annotation::SystemText(SystemText {
            text: rich_text(node, &style_format),
            placement: placement.unwrap_or(style_placement),
            visible: visible(node),
            swing: super::textlines::read_swing(node),
        }),
        "Tempo" => {
            // `TempoText::TempoText` part de 2 battements par seconde
            // (120 BPM) ; `<tempo>` l'écrase.
            let mut tempo = Tempo {
                text: rich_text(node, &style_format),
                beats_per_second: Some(
                    node.child("tempo")
                        .and_then(|child| child.trimmed_text().parse::<f64>().ok())
                        .unwrap_or(2.0),
                ),
                follow_text: node
                    .child("followText")
                    .and_then(Node::int_text)
                    .unwrap_or(0)
                    != 0,
                placement: placement.unwrap_or(style_placement),
                visible: visible(node),
                kind: TempoKind::Normal,
            };
            // `TRead::read(TempoText*)` : un tempo sans texte reçoit le texte
            // « ♩ = N » et devient invisible.
            if tempo.text.is_empty() {
                if let Some(beats_per_second) = tempo.beats_per_second {
                    let bpm = (beats_per_second * 60.0).round();
                    tempo.text = RichText::new(format!("<sym>metNoteQuarterUp</sym> = {bpm:.0}"));
                }
                tempo.visible = false;
            }
            Annotation::Tempo(tempo)
        }
        "RehearsalMark" => Annotation::RehearsalMark(RehearsalMark {
            text: rich_text(node, &style_format),
            placement: placement.unwrap_or(style_placement),
            frame: rehearsal_frame(node),
            visible: visible(node),
        }),
        "Harmony" => Annotation::Harmony(read_harmony(node, placement)),
        "Fermata" => super::notations::read_fermata(node, fermata_placement),
        "FiguredBass" => Annotation::FiguredBass(read_figured_bass(node)),
        "TremoloBar" => Annotation::TremoloBar(read_tremolo_bar(node)),
        "Expression" => Annotation::Expression(StaffText {
            text: rich_text(node, &style_format),
            placement: placement.unwrap_or(style_placement),
            visible: visible(node),
        }),
        "InstrumentChange" => {
            let instrument_node = node.child("Instrument");
            let instrument = instrument_node.map(|instrument_node| {
                let mut instrument = super::part::read_instrument(reader, instrument_node);
                // `Instrument::nameAsPlainText` garde le balisage des noms 3.x
                // (`<font size="…"/>Piano`), que `writeDisplayName` ré-échappe.
                for (tag, name) in [
                    ("longName", &mut instrument.long_name),
                    ("shortName", &mut instrument.short_name),
                ] {
                    if let Some(value) = instrument_node.child(tag) {
                        *name = value.inner_markup_raw_self_closing();
                    }
                }
                if instrument.id.is_empty() {
                    if let Some(id) = scoretk_core::model::recognize_musicxml_id(
                        &instrument.track_name,
                        instrument.midi_program,
                    ) {
                        instrument.id = id.to_owned();
                    }
                }
                instrument
            });
            Annotation::InstrumentChange(InstrumentChange {
                text: rich_text(node, &style_format),
                placement: placement.unwrap_or(style_placement),
                visible: visible(node),
                instrument,
            })
        }
        _ => return None,
    };
    Some(annotation)
}

/// Texte d'une annotation, balisage compris (`<b>`, `<font>`, `<sym>`…).
///
/// `format` est le format du style, éventuellement écrasé par les propriétés
/// `<italic>`, `<bold>`, `<underline>` et `<strike>` du texte.
pub(super) fn rich_text(node: &Node, style_format: &TextFormat) -> RichText {
    let mut format = style_format.clone();
    for (name, value) in [
        ("bold", &mut format.bold),
        ("italic", &mut format.italic),
        ("underline", &mut format.underline),
        ("strike", &mut format.strike),
    ] {
        if let Some(flag) = node.child(name).and_then(Node::int_text) {
            *value = flag != 0;
        }
    }
    if let Some(size) = node
        .child("size")
        .and_then(|child| child.trimmed_text().parse::<f64>().ok())
    {
        format.font_size = Some(size);
    }
    if let Some(family) = node.child("family").map(Node::trimmed_text) {
        if !family.is_empty() {
            format.font_family = Some(family);
        }
    }
    node.child("text")
        .map(|child| RichText::from_markup_with(&child.inner_markup(), &format))
        .unwrap_or_default()
}

/// Lit un `<FiguredBass>` (`TRead::read(FiguredBass*)`).
fn read_figured_bass(node: &Node) -> FiguredBass {
    let mut bass = FiguredBass::new();
    for child in node.children() {
        match child.name() {
            "ticks" => {
                bass.ticks = child.fraction().unwrap_or_default();
            }
            "FiguredBassItem" => {
                let mut item = FiguredBassItem::default();
                for field in child.children() {
                    match field.name() {
                        "prefix" => {
                            item.prefix = Some(FiguredBassModifier::from_mcsx(
                                field.int_text().unwrap_or(0),
                            ));
                        }
                        "digit" => {
                            item.digit = field
                                .int_text()
                                .and_then(|value| u8::try_from(value.clamp(0, 9)).ok());
                        }
                        "suffix" => {
                            item.suffix = Some(FiguredBassModifier::from_mcsx(
                                field.int_text().unwrap_or(0),
                            ));
                        }
                        "continuationLine" => {
                            item.cont_line =
                                FiguredBassContLine::from_mcsx(field.int_text().unwrap_or(0));
                        }
                        _ => {}
                    }
                }
                bass.items.push(item);
            }
            _ => {}
        }
    }
    bass.place_above = matches!(
        node.child("placement").map(Node::trimmed_text).as_deref(),
        Some("above")
    );
    bass.parentheses = node
        .child("parentheses")
        .and_then(Node::int_text)
        .is_some_and(|value| value != 0);
    bass.visible = visible(node);
    bass
}

/// Lit un `<TremoloBar>` (`TRead::read(TremoloBar*)`).
fn read_tremolo_bar(node: &Node) -> TremoloBar {
    let mut points = Vec::new();
    for child in node.children_named("point") {
        points.push(TremoloBarPoint {
            time: child
                .attr("time")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            pitch: child
                .attr("pitch")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
        });
    }
    let mut bar = TremoloBar::from_points(points);
    if let Some(kind) = node.child("type").and_then(Node::int_text) {
        bar.kind = match kind {
            1 => scoretk_core::model::TremoloBarType::Dive,
            2 => scoretk_core::model::TremoloBarType::ReleaseUp,
            3 => scoretk_core::model::TremoloBarType::InvertedDip,
            4 => scoretk_core::model::TremoloBarType::Return,
            5 => scoretk_core::model::TremoloBarType::ReleaseDown,
            6 => scoretk_core::model::TremoloBarType::Custom,
            _ => bar.kind,
        };
    }
    bar
}

/// Vrai si l'annotation n'est pas marquée `<visible>0</visible>`.
pub(super) fn visible(node: &Node) -> bool {
    node.child("visible")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0)
}

/// Cadre d'un repère (`<frameType>` : 0 sans cadre, 1 rectangle, 2 cercle).
fn rehearsal_frame(node: &Node) -> RehearsalFrame {
    match node.child("frameType").and_then(Node::int_text) {
        Some(0) => RehearsalFrame::NoFrame,
        Some(2) => RehearsalFrame::Circle,
        _ => RehearsalFrame::Rectangle,
    }
}

/// Lit un `<Harmony>`.
///
/// Le texte est reconstitué comme `Harmony::text()` : fondamentale (issue du
/// `tpc`), `<name>` puis basse si l'accord est renversé. Le champ `kind`
/// conserve le chiffrage interne (`normal`, `roman`, `nashville`) ; `name`,
/// `function` et les `<degree>` restent disponibles pour l'export, qui les
/// analyse comme `ParsedChord`.
fn read_harmony(node: &Node, placement: Option<Placement>) -> Harmony {
    let mut harmony = Harmony::new(String::new());
    let mut name = String::new();
    let mut custom_text: Option<String> = None;

    for child in node.children() {
        match child.name() {
            "root" => {
                harmony.root_tpc = child
                    .int_text()
                    .map_or(tpc::INVALID, |value| value.clamp(-128, 127) as i8);
            }
            "base" => {
                harmony.bass_tpc = Some(
                    child
                        .int_text()
                        .map_or(tpc::INVALID, |value| value.clamp(-128, 127) as i8),
                );
            }
            "name" => name = child.trimmed_text(),
            "function" => harmony.function = Some(child.trimmed_text()),
            "play" => harmony.play = child.int_text().is_none_or(|value| value != 0),
            "text" => custom_text = Some(child.text().trim().to_owned()),
            "degree" => {
                if let Some(degree) = read_harmony_degree(child) {
                    harmony.degrees.push(degree);
                }
            }
            "harmonyType" => {
                harmony.kind = match child.int_text() {
                    Some(0) | None => "normal",
                    Some(1) => "roman",
                    Some(2) => "nashville",
                    Some(_) => "normal",
                }
                .to_owned();
            }
            // TODO(parité): `<extension>`, `<rootCase>`/`<baseCase>` et
            // `<leftParen>`/`<rightParen>` ne sont pas modélisés.
            _ => {}
        }
    }

    if let Some(text) = custom_text {
        harmony.text = text;
    } else {
        let mut text = tables::tpc_name(harmony.root_tpc);
        text.push_str(&name);
        if let Some(bass) = harmony.bass_tpc {
            text.push('/');
            text.push_str(&tables::tpc_name(bass));
        }
        harmony.text = text;
    }
    harmony.name = name;
    if let Some(placement) = placement {
        harmony.placement = placement;
    }
    harmony
}

/// Lit un `<degree>` d'accord (`degree-value`, `degree-alter`, `degree-type`).
///
/// Reproduit les bornes de `TRead::read(Harmony*)` : un degré hors de `1..=13`,
/// une altération hors de `-2..=2` ou un type inconnu est ignoré.
fn read_harmony_degree(node: &Node) -> Option<scoretk_core::model::HarmonyDegree> {
    let mut value = 0u8;
    let mut alter = 0i8;
    let mut kind = None;
    for child in node.children() {
        match child.name() {
            "degree-value" => {
                value = child
                    .int_text()
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0);
            }
            "degree-alter" => {
                alter = child
                    .int_text()
                    .map_or(0, |value| value.clamp(-128, 127) as i8);
            }
            "degree-type" => {
                kind = match child.trimmed_text().as_str() {
                    "add" => Some(scoretk_core::model::HarmonyDegreeKind::Add),
                    "alter" => Some(scoretk_core::model::HarmonyDegreeKind::Alter),
                    "subtract" => Some(scoretk_core::model::HarmonyDegreeKind::Subtract),
                    _ => None,
                };
            }
            _ => {}
        }
    }
    let kind = kind?;
    if value == 0 || value > 13 || !(-2..=2).contains(&alter) {
        return None;
    }
    Some(scoretk_core::model::HarmonyDegree { value, alter, kind })
}
