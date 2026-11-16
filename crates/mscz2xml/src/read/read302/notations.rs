//! Notations des partitions 3.x : articulations, ornements, points d'orgue,
//! trémolos, arpèges, lignes d'accord, doigtés et détails des spanners.
//!
//! Portage des parties de `TRead::read(Articulation*)`, `Ornament*`,
//! `Fermata*`, `Tremolo*`, `Arpeggio*`, `ChordLine*` et `Fingering*`
//! (`engraving/rw/read400/tread.cpp`, utilisé aussi pour les fichiers 3.x) qui
//! alimentent l'export MusicXML.

use super::tables;
use super::xml::Node;
use crate::model::{
    Accidental, AccidentalType, Annotation, Arpeggio, Articulation, ArticulationAnchor,
    ArticulationType, ChordLineType, Fermata, FermataType, Fingering, Glissando, LineStyle,
    Ornament, OrnamentAccidental, Placement, Slur, Tremolo, TremoloKind, Trill,
};
use crate::read::notation_names as names;

/// Lit les articulations d'un `<Articulation>` (un élément peut en produire
/// plusieurs quand MuseScore éclate une articulation composée).
pub(crate) fn read_articulations(node: &Node) -> Vec<Articulation> {
    let explicit_anchor = articulation_anchor(node);
    let visible = node
        .child("visible")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0);
    let Some(subtype) = node.child("subtype").map(Node::trimmed_text) else {
        return Vec::new();
    };
    // `articulationNames2SymId206` accepte aussi les anciens noms : les noms
    // inconnus deviennent `Other`, avec leur ancre calculée par le suffixe.
    let parsed = names::articulations(&subtype);
    let kinds = if parsed.is_empty() {
        let (base, suffix_anchor) = names::split_anchor(&subtype);
        vec![(ArticulationType::Other(base.to_owned()), suffix_anchor)]
    } else {
        parsed
    };
    kinds
        .into_iter()
        .map(|(kind, anchor)| Articulation {
            kind,
            anchor: match explicit_anchor {
                ArticulationAnchor::Auto => anchor,
                explicit => explicit,
            },
            visible,
        })
        .collect()
}

/// Ancre explicite d'une articulation (`<anchor>` ou `<direction>`).
fn articulation_anchor(node: &Node) -> ArticulationAnchor {
    if let Some(anchor) = node.child("anchor").and_then(Node::int_text) {
        return tables::articulation_anchor(Some(anchor));
    }
    if let Some(direction) = node.child("direction") {
        return match direction.trimmed_text().as_str() {
            "up" => ArticulationAnchor::Up,
            "down" => ArticulationAnchor::Down,
            _ => ArticulationAnchor::Auto,
        };
    }
    tables::articulation_anchor(node.attr_i32("anchor"))
}

/// Point d'orgue d'un `<Fermata>`.
pub(crate) fn read_fermata(node: &Node) -> Annotation {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let kind = names::fermata_type(&subtype).unwrap_or(FermataType::Normal);
    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()))
        .unwrap_or(Placement::Above);
    let visible = node
        .child("visible")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0);
    Annotation::Fermata(Fermata {
        kind,
        placement,
        visible,
    })
}

/// Ornement d'un `<Ornament>` (sous-classe d'articulation en 3.x).
pub(crate) fn read_ornament(node: &Node) -> Ornament {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let mut ornament = Ornament::new(names::ornament_type(&subtype));
    ornament.placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()));
    ornament.visible = node
        .child("visible")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0);
    for accidental in node.children_named("Accidental") {
        let subtype = accidental
            .child("subtype")
            .map(Node::trimmed_text)
            .unwrap_or_default();
        let kind = tables::accidental_type(&subtype);
        if kind == AccidentalType::None {
            continue;
        }
        let placement = accidental
            .child("placement")
            .map(|child| tables::placement(&child.trimmed_text()))
            .unwrap_or(Placement::Below);
        let mut value = Accidental::new(kind);
        if let Some(bracket) = accidental.child("bracket").and_then(Node::int_text) {
            value.bracket = bracket != 0;
        }
        if let Some(parentheses) = accidental.child("parentheses").and_then(Node::int_text) {
            value.parentheses = parentheses != 0;
        }
        ornament.accidentals.push(OrnamentAccidental {
            accidental: value,
            placement,
        });
    }
    ornament
}

/// Trémolo d'un `<Tremolo>` enfant d'accord.
pub(crate) fn read_tremolo(node: &Node) -> Option<Tremolo> {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let (double, count) = names::tremolo_bars(&subtype)?;
    Some(Tremolo {
        kind: if double {
            TremoloKind::Start
        } else {
            TremoloKind::Single
        },
        count,
    })
}

/// Arpège d'un `<Arpeggio>` enfant d'accord.
pub(crate) fn read_arpeggio(node: &Node) -> Arpeggio {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    Arpeggio::new(names::arpeggio_type(&subtype))
}

/// Ligne d'accord d'un `<ChordLine>` enfant de note.
pub(crate) fn read_chord_line(node: &Node) -> Option<ChordLineType> {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    names::chord_line_type(&subtype)
}

/// Doigté d'un `<Fingering>` enfant de note.
pub(crate) fn read_fingering(node: &Node) -> Option<Fingering> {
    let text = node.child("text").map(Node::inner_markup)?;
    let text = text.trim().to_owned();
    if text.is_empty() {
        return None;
    }
    Some(Fingering {
        text,
        placement: node
            .child("placement")
            .map(|child| tables::placement(&child.trimmed_text())),
        visible: node
            .child("visible")
            .and_then(Node::int_text)
            .is_none_or(|value| value != 0),
    })
}

/// Détails d'un `<Slur>` (`Spanner type="Slur"`).
pub(crate) fn read_slur(node: &Node) -> Slur {
    let mut slur = Slur::new();
    if let Some(up) = node.child("up") {
        slur.placement = match up.trimmed_text().as_str() {
            "up" => Some(Placement::Above),
            "down" => Some(Placement::Below),
            _ => None,
        };
    }
    slur.visible = node
        .child("visible")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0);
    slur
}

/// Détails d'un `<Glissando>` (`Spanner type="Glissando"`).
pub(crate) fn read_glissando(node: &Node) -> Glissando {
    let style = node
        .child("lineStyle")
        .map(|child| names::line_style(&child.trimmed_text()))
        .unwrap_or(LineStyle::Solid);
    let subtype = node.child("subtype").map(Node::trimmed_text);
    let mut glissando = Glissando::new(names::glissando_type(subtype.as_deref(), style));
    glissando.text = node.child("text").map(Node::text).unwrap_or_default();
    glissando.show_text = node
        .child("showText")
        .and_then(Node::int_text)
        .is_none_or(|value| value != 0);
    glissando
}

/// Détails d'un `<Trill>` (`Spanner type="Trill"`).
pub(crate) fn read_trill(node: &Node) -> Trill {
    let subtype = node
        .child("subtype")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let mut trill = Trill::new(names::trill_type(&subtype));
    if let Some(accidental) = node.child("Accidental") {
        let subtype = accidental
            .child("subtype")
            .map(Node::trimmed_text)
            .unwrap_or_default();
        let kind = tables::accidental_type(&subtype);
        if kind != AccidentalType::None {
            let mut value = Accidental::new(kind);
            if let Some(bracket) = accidental.child("bracket").and_then(Node::int_text) {
                value.bracket = bracket != 0;
            }
            if let Some(parentheses) = accidental.child("parentheses").and_then(Node::int_text) {
                value.parentheses = parentheses != 0;
            }
            trill.accidental = Some(value);
        }
    }
    trill
}
