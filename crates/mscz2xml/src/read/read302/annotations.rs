//! Annotations de segment : nuances, textes, tempos, repères et accords.
//!
//! Portage partiel de `TRead::read(Dynamic*)`, `read(StaffText*)`,
//! `read(SystemText*)`, `read(TempoText*)`, `read(RehearsalMark*)` et
//! `read(Harmony*)`. Les éléments hors noyau sont ignorés par l'appelant.

use super::tables;
use super::xml::Node;
use crate::model::{
    tpc, Annotation, Dynamic, Harmony, Placement, RehearsalMark, StaffText, SystemText, Tempo,
};

/// Lit une annotation de segment, ou `None` si l'élément n'en est pas une.
pub(crate) fn read(node: &Node) -> Option<Annotation> {
    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()));

    let annotation = match node.name() {
        "Dynamic" => {
            let mut dynamic = Dynamic::new(text_of(node, "text"));
            if dynamic.text.is_empty() {
                // `<subtype>other-dynamics</subtype>` sans texte : MuseScore
                // affiche alors le nom du sous-type.
                dynamic.text = node
                    .child("subtype")
                    .map(Node::trimmed_text)
                    .unwrap_or_default();
            }
            dynamic.velocity = node
                .child("velocity")
                .and_then(Node::int_text)
                .and_then(|value| u8::try_from(value.clamp(0, 127)).ok());
            dynamic.placement = placement.unwrap_or(Placement::Below);
            Annotation::Dynamic(dynamic)
        }
        "StaffText" | "Text" => Annotation::StaffText(StaffText {
            text: text_of(node, "text"),
            placement: placement.unwrap_or(Placement::Above),
        }),
        "SystemText" => Annotation::SystemText(SystemText {
            text: text_of(node, "text"),
            placement: placement.unwrap_or(Placement::Above),
        }),
        "Tempo" => {
            let mut tempo = Tempo::new(text_of(node, "text"));
            tempo.beats_per_second = node
                .child("tempo")
                .and_then(|child| child.trimmed_text().parse::<f64>().ok());
            tempo.follow_text = node
                .child("followText")
                .and_then(Node::int_text)
                .unwrap_or(0)
                != 0;
            tempo.placement = placement.unwrap_or(Placement::Above);
            Annotation::Tempo(tempo)
        }
        "RehearsalMark" => Annotation::RehearsalMark(RehearsalMark {
            text: text_of(node, "text"),
            placement: placement.unwrap_or(Placement::Above),
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
        "Expression" => Annotation::Other {
            kind: "Expression".to_owned(),
            text: text_of(node, "text"),
            placement: placement.unwrap_or(Placement::Below),
        },
        _ => return None,
    };
    Some(annotation)
}

fn text_of(node: &Node, child: &str) -> String {
    node.child(child)
        .map(Node::text)
        .unwrap_or_default()
        .trim()
        .to_owned()
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
