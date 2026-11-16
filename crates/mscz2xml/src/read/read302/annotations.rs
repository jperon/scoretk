//! Annotations de segment : nuances, textes, tempos, repères et accords.
//!
//! Portage partiel de `TRead::read(Dynamic*)`, `read(StaffText*)`,
//! `read(SystemText*)`, `read(TempoText*)`, `read(RehearsalMark*)` et
//! `read(Harmony*)`. Les éléments hors noyau sont ignorés par l'appelant.

use super::tables;
use super::xml::Node;
use super::Reader;
use crate::model::{
    text_style_defaults, tpc, Annotation, Harmony, InstrumentChange, Placement, RehearsalFrame,
    RehearsalMark, RichText, StaffText, SystemText, Tempo, TempoKind, TextFormat,
};

/// Lit une annotation de segment, ou `None` si l'élément n'en est pas une.
pub(crate) fn read(reader: &Reader, node: &Node) -> Option<Annotation> {
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
                    if let Some(id) = crate::model::recognize_musicxml_id(
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
fn rich_text(node: &Node, style_format: &TextFormat) -> RichText {
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

/// Vrai si l'annotation n'est pas marquée `<visible>0</visible>`.
fn visible(node: &Node) -> bool {
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
/// conserve le chiffrage interne (`normal`, `roman`, `nashville`) et les
/// degrés ne sont pas modélisés.
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
            "text" => custom_text = Some(child.text().trim().to_owned()),
            "harmonyType" => {
                harmony.kind = match child.int_text() {
                    Some(0) | None => "normal",
                    Some(1) => "roman",
                    Some(2) => "nashville",
                    Some(_) => "normal",
                }
                .to_owned();
            }
            // TODO(parité): `<function>`, `<degree>`, `<extension>`,
            // `<rootCase>`/`<baseCase>` et `<leftParen>`/`<rightParen>` ne sont
            // pas modélisés ; le texte reste celui de `<name>`.
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
    if let Some(placement) = placement {
        harmony.placement = placement;
    }
    harmony
}
