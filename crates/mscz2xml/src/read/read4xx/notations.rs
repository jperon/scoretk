//! Notations du lecteur 4.x : articulations, ornements, points d'orgue,
//! trémolos, arpèges, lignes d'accord et doigtés.
//!
//! Portage des parties de `TRead::read(Articulation*)`, `Ornament*`,
//! `Fermata*`, `Tremolo*`, `TremoloTwoChord*`, `Arpeggio*`, `ChordLine*` et
//! `Fingering*` (`engraving/rw/read400/tread.cpp` et suivants) utiles à
//! l'export MusicXML.

use super::dom::Element;
use super::placement;
use crate::model::{
    Accidental, AccidentalType, Annotation, Arpeggio, Articulation, ArticulationAnchor,
    ArticulationType, ChordLineType, Fermata, FermataType, Fingering, OrnamentAccidental,
    Placement, Tremolo, TremoloKind,
};
use crate::read::notation_names as names;

/// Lit un `<Articulation>` ; une articulation composée en produit plusieurs.
pub(crate) fn read_articulations(element: &Element) -> Vec<Articulation> {
    let Some(subtype) = element.child("subtype").map(Element::text) else {
        return Vec::new();
    };
    let visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    let explicit_anchor = if let Some(anchor) = element.child("anchor").and_then(Element::int) {
        match anchor {
            1 => ArticulationAnchor::Up,
            2 => ArticulationAnchor::Down,
            _ => ArticulationAnchor::Auto,
        }
    } else if let Some(direction) = element.child("direction").map(Element::text) {
        match direction {
            "up" => ArticulationAnchor::Up,
            "down" => ArticulationAnchor::Down,
            _ => ArticulationAnchor::Auto,
        }
    } else {
        ArticulationAnchor::Auto
    };

    // L'ancre du suffixe ne fixe pas `ArticulationAnchor` dans MuseScore :
    // elle donne seulement le sens du glyphe (`Articulation::up()`).
    let (_, suffix_anchor) = names::split_anchor(subtype);
    let up = suffix_anchor != ArticulationAnchor::Down;
    let parsed = names::articulations(subtype);
    let kinds = if parsed.is_empty() {
        vec![(ArticulationType::Other(subtype.to_owned()), suffix_anchor)]
    } else {
        parsed
    };
    kinds
        .into_iter()
        .map(|(kind, _)| Articulation {
            kind,
            anchor: explicit_anchor,
            visible,
            up,
        })
        .collect()
}

/// Lit un `<Ornament>` enfant d'accord.
pub(crate) fn read_ornament(element: &Element) -> Option<crate::model::Ornament> {
    let subtype = element.child("subtype").map(Element::text)?;
    let mut ornament = crate::model::Ornament::new(names::ornament_type(subtype));
    ornament.placement = element
        .child("placement")
        .map(|child| placement(Some(child)));
    ornament.visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    for accidental in element.children_named("Accidental") {
        let Some(subtype) = accidental.child("subtype").map(Element::text) else {
            continue;
        };
        let Some(kind) = accidental_type(subtype) else {
            continue;
        };
        let mut value = Accidental::new(kind);
        if let Some(bracket) = accidental.child("bracket").and_then(Element::int) {
            value.bracket = bracket != 0;
        }
        if let Some(parentheses) = accidental.child("parentheses").and_then(Element::int) {
            value.parentheses = parentheses != 0;
        }
        ornament.accidentals.push(OrnamentAccidental {
            accidental: value,
            placement: accidental
                .child("placement")
                .map(|child| placement(Some(child)))
                .unwrap_or(Placement::Below),
        });
    }
    Some(ornament)
}

/// Altération nommée d'un ornement ou d'un trille.
fn accidental_type(name: &str) -> Option<AccidentalType> {
    Some(match name.trim() {
        "accidentalFlat" => AccidentalType::Flat,
        "accidentalNatural" => AccidentalType::Natural,
        "accidentalSharp" => AccidentalType::Sharp,
        "accidentalDoubleFlat" => AccidentalType::DoubleFlat,
        "accidentalDoubleSharp" => AccidentalType::DoubleSharp,
        "accidentalNaturalFlat" => AccidentalType::NaturalFlat,
        "accidentalNaturalSharp" => AccidentalType::NaturalSharp,
        "accidentalTripleFlat" => AccidentalType::TripleFlat,
        "accidentalTripleSharp" => AccidentalType::TripleSharp,
        _ => return None,
    })
}

/// Lit un `<Fermata>` et le range dans les annotations de segment.
pub(crate) fn read_fermata(element: &Element) -> Annotation {
    let subtype = element
        .child("subtype")
        .map(Element::text)
        .unwrap_or_default();
    let kind = names::fermata_type(subtype).unwrap_or(FermataType::Normal);
    Annotation::Fermata(Fermata {
        kind,
        placement: element
            .child("placement")
            .map(|child| placement(Some(child)))
            .unwrap_or(Placement::Above),
        visible: !element
            .child("visible")
            .and_then(Element::int)
            .is_some_and(|value| value == 0),
    })
}

/// Lit un `<Tremolo>` (trémolo simple).
pub(crate) fn read_tremolo(element: &Element) -> Option<Tremolo> {
    let subtype = element.child("subtype").map(Element::text)?;
    let (double, count) = names::tremolo_bars(subtype)?;
    Some(Tremolo {
        kind: if double {
            TremoloKind::Start
        } else {
            TremoloKind::Single
        },
        count,
    })
}

/// Lit un `<TremoloTwoChord>` (premier accord d'un trémolo à deux notes).
pub(crate) fn read_tremolo_two_chord(element: &Element) -> Option<Tremolo> {
    let subtype = element.child("subtype").map(Element::text)?;
    let (_, count) = names::tremolo_bars(subtype)?;
    Some(Tremolo {
        kind: TremoloKind::Start,
        count,
    })
}

/// Lit un `<Arpeggio>`.
pub(crate) fn read_arpeggio(element: &Element) -> Arpeggio {
    let subtype = element
        .child("subtype")
        .map(Element::text)
        .unwrap_or_default();
    let mut arpeggio = Arpeggio::new(names::arpeggio_type(subtype));
    arpeggio.visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    arpeggio
}

/// Lit un `<ChordLine>` (enfant de note).
pub(crate) fn read_chord_line(element: &Element) -> Option<ChordLineType> {
    let subtype = element.child("subtype").map(Element::text)?;
    names::chord_line_type(subtype)
}

/// Lit un `<Fingering>` (enfant de note).
pub(crate) fn read_fingering(element: &Element) -> Option<Fingering> {
    let text = element.child("text")?.text.trim();
    if text.is_empty() {
        return None;
    }
    Some(Fingering {
        text: text.to_owned(),
        placement: element
            .child("placement")
            .map(|child| placement(Some(child))),
        visible: !element
            .child("visible")
            .and_then(Element::int)
            .is_some_and(|value| value == 0),
    })
}

//----------------------------------------------------------------------
// Spanners de notation : liaisons d'expression, trilles, glissandos
//----------------------------------------------------------------------

use super::dom::Element as Node;
use crate::model::{
    Fraction, Glissando, LineStyle, Measure, Slur, Spanner, SpannerAnchor, SpannerNotation,
    SpannerType, Tick, Track, Trill, VOICES,
};

/// Type de notation porté par une moitié de spanner.
#[derive(Clone)]
pub(crate) enum Notation {
    /// Liaison d'expression.
    Slur(Slur),
    /// Trille.
    Trill(Trill),
    /// Glissando.
    Glissando(Glissando),
}

/// `<location>` relative d'une extrémité de spanner.
#[derive(Clone, Copy, Default)]
struct Location {
    measures: i64,
    fractions: Fraction,
    staves: i64,
    voices: i64,
}

impl Location {
    /// Position absolue de l'extrémité, comme `Location::toAbsolute`.
    fn absolute(self, tick: Tick, track: Track, measures: &[Measure]) -> (Tick, Track) {
        let current = measures
            .iter()
            .rposition(|measure| measure.start_tick <= tick)
            .unwrap_or(0);
        let target = (current as i64 + self.measures)
            .clamp(0, measures.len().saturating_sub(1) as i64) as usize;
        let base = measures
            .get(target)
            .map_or(Fraction::ZERO, |m| m.start_tick);
        let current_base = measures
            .get(current)
            .map_or(Fraction::ZERO, |m| m.start_tick);
        let raw_track = i64::from(track.raw()) + self.voices + self.staves * VOICES as i64;
        (
            base + (tick - current_base) + self.fractions,
            Track::from_raw(raw_track.max(0) as u32),
        )
    }
}

/// Moitié de spanner de notation en attente d'appariement.
pub(crate) struct Half {
    tick: Tick,
    track: Track,
    prev: Option<Location>,
    next: Option<Location>,
    notation: Option<Notation>,
}

/// Lit un `<Spanner>` de notation rencontré dans une voix, un accord ou une
/// note.
pub(crate) fn read_spanner(reader: &mut super::Reader, element: &Node, tick: Tick, track: Track) {
    let Some(kind) = element.attr("type") else {
        return;
    };
    if !matches!(kind, "Slur" | "Trill" | "Glissando") {
        return;
    }
    // La moitié de fin n'a pas d'enfant portant le nom du type : ses données
    // viennent de la moitié de départ.
    let notation = element.child(kind).map(|payload| match kind {
        "Slur" => Notation::Slur(read_slur(payload)),
        "Trill" => Notation::Trill(read_trill_payload(payload)),
        _ => Notation::Glissando(read_glissando(payload)),
    });
    let mut half = Half {
        tick,
        track,
        prev: None,
        next: None,
        notation,
    };
    for child in &element.children {
        match child.name.as_str() {
            "prev" => half.prev = child.child("location").map(read_location),
            "next" => half.next = child.child("location").map(read_location),
            _ => {}
        }
    }
    reader.notation_halves.push(half);
}

fn read_location(element: &Node) -> Location {
    let mut location = Location::default();
    for child in &element.children {
        match child.name.as_str() {
            "measures" => location.measures = child.int().unwrap_or(0),
            "fractions" => location.fractions = child.fraction().unwrap_or(Fraction::ZERO),
            "staves" => location.staves = child.int().unwrap_or(0),
            "voices" => location.voices = child.int().unwrap_or(0),
            _ => {}
        }
    }
    location
}

fn read_slur(element: &Node) -> Slur {
    let mut slur = Slur::new();
    if let Some(up) = element.child("up").map(Element::text) {
        slur.placement = match up {
            "up" => Some(Placement::Above),
            "down" => Some(Placement::Below),
            _ => None,
        };
    }
    slur.visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    slur
}

fn read_trill_payload(element: &Node) -> Trill {
    let subtype = element
        .child("subtype")
        .map(Element::text)
        .unwrap_or_default();
    let mut trill = Trill::new(names::trill_type(subtype));
    if let Some(accidental) = element.child("Accidental") {
        if let Some(subtype) = accidental.child("subtype").map(Element::text) {
            if let Some(kind) = accidental_type(subtype) {
                trill.accidental = Some(Accidental::new(kind));
            }
        }
    }
    trill
}

fn read_glissando(element: &Node) -> Glissando {
    let style = element
        .child("lineStyle")
        .map(|node| names::line_style(Element::text(node)))
        .unwrap_or(LineStyle::Solid);
    let subtype = element.child("subtype").map(Element::text);
    let mut glissando = Glissando::new(names::glissando_type(subtype, style));
    glissando.text = element
        .child("text")
        .map(|node| node.text.clone())
        .unwrap_or_default();
    glissando.show_text = !element
        .child("showText")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    glissando.visible = !element
        .child("visible")
        .and_then(Element::int)
        .is_some_and(|value| value == 0);
    glissando
}

/// Apparie les moitiés et ajoute les spanners à la partition.
pub(crate) fn resolve(reader: &mut super::Reader) {
    let halves = std::mem::take(&mut reader.notation_halves);
    let measures = &reader.score.measures;
    let mut spanners: Vec<Spanner> = Vec::new();

    for (index, first) in halves.iter().enumerate() {
        let Some(next) = first.next else {
            continue;
        };
        let start = SpannerAnchor::new(first.tick, first.track);
        let (end_tick, end_track) = next.absolute(first.tick, first.track, measures);
        for (other_index, second) in halves.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let Some(prev) = second.prev else {
                continue;
            };
            let (prev_tick, prev_track) = prev.absolute(second.tick, second.track, measures);
            if prev_tick != start.tick || prev_track != start.track {
                continue;
            }
            if second.tick != end_tick || second.track != end_track {
                continue;
            }
            let Some(notation) = first.notation.as_ref().or(second.notation.as_ref()) else {
                continue;
            };
            let kind = match notation {
                Notation::Slur(_) => SpannerType::Slur,
                Notation::Trill(_) => SpannerType::Trill,
                Notation::Glissando(_) => SpannerType::Glissando,
            };
            let mut spanner =
                Spanner::new(kind, start, SpannerAnchor::new(second.tick, second.track));
            spanner.notation = Some(match notation {
                Notation::Slur(slur) => SpannerNotation::Slur(slur.clone()),
                Notation::Trill(trill) => SpannerNotation::Trill(trill.clone()),
                Notation::Glissando(glissando) => SpannerNotation::Glissando(glissando.clone()),
            });
            spanners.push(spanner);
            break;
        }
    }

    reader.score.spanners.extend(spanners);
}

/// Marque le second accord d'un trémolo à deux notes comme `stop`.
pub(crate) fn tremolo_stop(count: u8) -> Tremolo {
    Tremolo {
        kind: TremoloKind::Stop,
        count,
    }
}
