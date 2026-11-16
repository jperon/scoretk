//! Annotations de segment : nuances, textes, tempos, repères et accords.
//!
//! Portage partiel de `TRead::read(Dynamic*)`, `read(StaffText*)`,
//! `read(SystemText*)`, `read(TempoText*)`, `read(RehearsalMark*)` et
//! `read(Harmony*)`. Les éléments hors noyau sont ignorés par l'appelant.

use super::tables;
use super::xml::Node;
use crate::model::{
    text_style_defaults, tpc, Annotation, Harmony, Placement, RehearsalFrame, RehearsalMark,
    RichText, StaffText, SystemText, Tempo, TextFormat,
};

/// Lit une annotation de segment, ou `None` si l'élément n'en est pas une.
pub(crate) fn read(node: &Node) -> Option<Annotation> {
    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()));
    // Le style fixe le placement et la police de départ ; les propriétés
    // `italic`/`bold`/`underline`/`strike` du texte les écrasent ensuite.
    let (style_placement, style_format) = text_style_defaults(
        node.child("style")
            .map(|child| child.trimmed_text())
            .unwrap_or_default()
            .as_str(),
    );

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
        "Fermata" => Annotation::Other {
            kind: "Fermata".to_owned(),
            text: node
                .child("subtype")
                .map(Node::trimmed_text)
                .unwrap_or_default(),
            placement: placement.unwrap_or(Placement::Above),
        },
        "Expression" => Annotation::Expression(StaffText {
            text: rich_text(node, &style_format),
            placement: placement.unwrap_or(style_placement),
            visible: visible(node),
        }),
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
    node.child("text")
        .map(|child| RichText::from_markup_with(&child.inner_markup(), &format))
        .unwrap_or_default()
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
fn read_harmony_degree(node: &Node) -> Option<crate::model::HarmonyDegree> {
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
                    "add" => Some(crate::model::HarmonyDegreeKind::Add),
                    "alter" => Some(crate::model::HarmonyDegreeKind::Alter),
                    "subtract" => Some(crate::model::HarmonyDegreeKind::Subtract),
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
    Some(crate::model::HarmonyDegree { value, alter, kind })
}
