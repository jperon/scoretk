//! Reprises à l'export : barres de mesure, voltas, marqueurs et sauts.
//!
//! Portage de `ExportMusicXml::barlineLeft`, `barlineMiddle`, `barlineRight`,
//! `ending`, `repeatAtMeasureStart`, `repeatAtMeasureStop`, `directionMarker`
//! et `directionJump`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp`).
//!
//! Les attributs de mise en page (`default-x`, `default-y`, `relative-x`,
//! `relative-y`, couleurs, polices) ne sont pas dans le modèle : ils ne sont
//! pas écrits.

use crate::model::{
    BarLine, BarLineType, Jump, Marker, MarkerType, Measure, Placement, RepeatMark, Score, Spanner,
    SpannerType, StaffId, Track,
};

use super::exportmusicxml::{Context, Out};

/// Style de barre pour une barre de fin (`normalBarlineStyle` côté C++).
fn barline_style(kind: BarLineType) -> Option<&'static str> {
    match kind {
        BarLineType::Normal => Some("regular"),
        BarLineType::Double => Some("light-light"),
        BarLineType::Broken | BarLineType::Dashed => Some("dashed"),
        BarLineType::End | BarLineType::EndRepeat | BarLineType::EndStartRepeat => {
            Some("light-heavy")
        }
        BarLineType::Dotted => Some("dotted"),
        // MuseScore ne produit pas de style pour cette variante.
        BarLineType::DoubleDotted => None,
        BarLineType::Short | BarLineType::DoubleShort => Some("short"),
        BarLineType::Tick => Some("tick"),
        BarLineType::StartRepeat => None,
    }
}

/// `shortBarlineStyle` : barre courte ou en tick déduite des extensions.
fn short_barline_style(bar_line: &BarLine) -> Option<&'static str> {
    if bar_line.kind != BarLineType::Normal || bar_line.span_staff {
        return None;
    }
    match (bar_line.span_from, bar_line.span_to) {
        (2, -2) | (1, -1) => Some("short"),
        (-1, -7) | (-2, -6) => Some("tick"),
        _ => None,
    }
}

fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Above => "above",
        Placement::Below => "below",
    }
}

/// Première portée d'une partie.
fn first_staff(context: &Context<'_>, part_index: usize) -> Option<StaffId> {
    context
        .score
        .parts
        .get(part_index)
        .and_then(|part| part.staves.first().copied())
}

/// Première piste d'une partie (`strack` du C++).
fn first_track(context: &Context<'_>, part_index: usize) -> Option<Track> {
    first_staff(context, part_index).map(|staff| Track::new(staff, 0))
}

/// Cherche la volta qui commence (`left`) ou finit (`right`) dans la mesure.
fn find_volta<'a>(
    context: &Context<'a>,
    measure: &Measure,
    left: bool,
    staff: StaffId,
) -> Option<&'a Spanner> {
    context.score.spanners.iter().find(|spanner| {
        spanner.kind == SpannerType::Volta
            && spanner.start.track.staff() == staff
            && if left {
                spanner.start.tick == measure.start_tick
            } else {
                spanner.end.tick == measure.end_tick()
            }
    })
}

/// Écrit `<ending>` (`ending`).
fn ending(out: &mut Out, volta: &Spanner, left: bool) {
    let info = volta.volta.as_ref();
    let number = info
        .map(|info| {
            info.endings
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let closed = info.is_some_and(|info| info.closed);
    if left {
        out.xml.tag_with(
            "ending",
            &[("number", &number), ("type", "start")],
            volta.text.as_deref().unwrap_or(""),
        );
    } else {
        let kind = if closed { "stop" } else { "discontinue" };
        out.xml
            .tag_empty("ending", &[("number", &number), ("type", kind)]);
    }
}

/// Écrit la barre de gauche (`barlineLeft`).
pub(super) fn barline_left(
    context: &Context<'_>,
    out: &mut Out,
    measure: &Measure,
    part_index: usize,
) {
    let repeat = measure.start_bar_line.is_repeat_start();
    let volta = first_staff(context, part_index)
        .and_then(|staff| find_volta(context, measure, true, staff));
    if !repeat && volta.is_none() {
        return;
    }
    out.attr(false);
    out.xml.start_element("barline", &[("location", "left")]);
    if repeat {
        out.xml.tag("bar-style", "heavy-light");
    }
    if let Some(volta) = volta {
        ending(out, volta, true);
    }
    if repeat {
        out.xml.tag_empty("repeat", &[("direction", "forward")]);
    }
    out.xml.end_element();
}

/// Écrit une barre du milieu de mesure (`barlineMiddle`).
pub(super) fn barline_middle(out: &mut Out, bar_line: &BarLine) {
    let style = short_barline_style(bar_line).or_else(|| barline_style(bar_line.kind));
    if let Some(style) = style {
        out.attr(false);
        out.xml.start_element("barline", &[("location", "middle")]);
        out.xml.tag("bar-style", style);
        out.xml.end_element();
    }
}

/// Barre de fin effective.
///
/// MuseScore termine toujours la dernière mesure du score par une barre
/// finale (`light-heavy`), même quand le `.mscx` ne contient aucun `<BarLine>`.
fn end_bar_line_type(
    context: &Context<'_>,
    measure: &Measure,
    measure_index: usize,
) -> BarLineType {
    if measure.end_bar_line == BarLineType::Normal
        && measure_index + 1 == context.score.measures.len()
    {
        BarLineType::End
    } else {
        measure.end_bar_line
    }
}

/// Barre de fin portée par la mesure, si le `.mscx` en déclare une.
fn end_bar_line(measure: &Measure) -> Option<&BarLine> {
    let tick = measure.end_tick();
    measure
        .segments
        .iter()
        .filter(|segment| segment.kind == crate::model::SegmentType::EndBarLine)
        .find(|segment| segment.tick == tick)
        .and_then(|segment| {
            segment.iter_elements().find_map(|(_, item)| match item {
                crate::model::SegmentItem::BarLine(bar_line) => Some(bar_line),
                _ => None,
            })
        })
}

/// Écrit la barre de droite (`barlineRight`).
pub(super) fn barline_right(
    context: &Context<'_>,
    out: &mut Out,
    measure: &Measure,
    part_index: usize,
    measure_index: usize,
) {
    let kind = end_bar_line_type(context, measure, measure_index);
    let need_style = kind != BarLineType::Normal && kind != BarLineType::StartRepeat;
    let volta = first_staff(context, part_index)
        .and_then(|staff| find_volta(context, measure, false, staff));
    let bar_line = end_bar_line(measure);
    let special = bar_line.and_then(short_barline_style);

    if !need_style && volta.is_none() && special.is_none() {
        return;
    }

    out.attr(false);
    out.xml.start_element("barline", &[("location", "right")]);
    if need_style {
        if let Some(style) = barline_style(kind) {
            out.xml.tag("bar-style", style);
        }
    } else if let Some(style) = special {
        out.xml.tag("bar-style", style);
    }
    if let Some(volta) = volta {
        ending(out, volta, false);
    }
    if kind.is_repeat_end() {
        let mut attributes: Vec<(&str, &str)> = vec![("direction", "backward")];
        let times = measure.repeat_count.to_string();
        if measure.repeat_count > 2 {
            attributes.push(("times", &times));
        }
        out.xml.tag_empty("repeat", &attributes);
    }
    out.xml.end_element();
}

// ----------------------------------------------------------------------
// Marqueurs et sauts
// ----------------------------------------------------------------------

/// Tous les sauts de la partition, dans l'ordre des mesures.
fn jumps(score: &Score) -> Vec<&Jump> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.repeat_marks.iter())
        .filter_map(|mark| match mark {
            RepeatMark::Jump(jump) => Some(jump),
            RepeatMark::Marker(_) => None,
        })
        .collect()
}

/// Devine le type d'un marqueur personnalisé (`getEffectiveMarkerType`).
fn effective_marker_type(marker: &Marker, jumps: &[&Jump]) -> MarkerType {
    if marker.kind != MarkerType::User {
        return marker.kind;
    }
    let label = marker.label.as_str();
    let mut kind = MarkerType::User;
    for jump in jumps {
        let guessed = if jump.jump_to == label {
            MarkerType::Segno
        } else if jump.play_until == label {
            if jump.continue_at.is_empty() {
                MarkerType::Fine
            } else {
                MarkerType::ToCoda
            }
        } else if jump.continue_at == label {
            MarkerType::Coda
        } else {
            kind
        };
        if guessed != kind {
            if kind != MarkerType::User {
                // Les sauts ne s'accordent pas sur le type : le C++ laisse
                // `USER`, qui n'est pas exporté.
                return MarkerType::User;
            }
            kind = guessed;
        }
    }
    kind
}

/// Cible de coda associée à un « To Coda » (`findCodaLabel`).
fn find_coda_label<'a>(jumps: &[&'a Jump], to_coda_label: &str) -> Option<&'a str> {
    jumps
        .iter()
        .find(|jump| jump.play_until == to_coda_label)
        .map(|jump| jump.continue_at.as_str())
        .filter(|label| !label.is_empty())
}

fn sound(out: &mut Out, attributes: &[(&str, &str)]) {
    out.xml.tag_empty("sound", attributes);
}

/// Écrit un marqueur (`directionMarker`).
fn direction_marker(out: &mut Out, marker: &Marker, kind: MarkerType, jumps: &[&Jump]) {
    let placement = placement_name(marker.placement);
    match kind {
        MarkerType::VarSegno | MarkerType::Segno => {
            out.xml
                .start_element("direction", &[("placement", placement)]);
            out.xml.start_element("direction-type", &[]);
            if kind == MarkerType::VarSegno {
                out.xml.tag_empty("segno", &[("smufl", "segnoSerpent1")]);
            } else {
                out.xml.tag_empty("segno", &[]);
            }
            out.xml.end_element();
            let label = if marker.label.is_empty() {
                "1"
            } else {
                marker.label.as_str()
            };
            sound(out, &[("segno", label)]);
            out.xml.end_element();
        }
        MarkerType::VarCoda | MarkerType::Coda | MarkerType::Codetta => {
            out.xml
                .start_element("direction", &[("placement", placement)]);
            out.xml.start_element("direction-type", &[]);
            if kind == MarkerType::VarCoda {
                out.xml.tag_empty("coda", &[("smufl", "codaSquare")]);
            } else {
                out.xml.tag_empty("coda", &[]);
            }
            out.xml.end_element();
            let label = if marker.label.is_empty() {
                "1"
            } else {
                marker.label.as_str()
            };
            sound(out, &[("coda", label)]);
            out.xml.end_element();
        }
        MarkerType::Fine => {
            out.xml
                .start_element("direction", &[("placement", placement)]);
            out.xml.start_element("direction-type", &[]);
            out.xml.tag("words", &marker.text);
            out.xml.end_element();
            sound(out, &[("fine", "yes")]);
            out.xml.end_element();
        }
        MarkerType::ToCoda
        | MarkerType::ToCodaSym
        | MarkerType::DaCoda
        | MarkerType::DaDoubleCoda => {
            out.xml
                .start_element("direction", &[("placement", placement)]);
            out.xml.start_element("direction-type", &[]);
            let words = if marker.text.is_empty() {
                "To Coda"
            } else {
                marker.text.as_str()
            };
            out.xml.tag("words", words);
            out.xml.end_element();
            let label = find_coda_label(jumps, &marker.label).unwrap_or("1");
            sound(out, &[("tocoda", label)]);
            out.xml.end_element();
        }
        MarkerType::User => {}
    }
}

/// Écrit un saut (`directionJump`).
fn direction_jump(out: &mut Out, jump: &Jump) {
    let words = if !jump.text.is_empty() {
        jump.text.as_str()
    } else {
        match jump.kind {
            crate::model::JumpType::Dc => "D.C.",
            crate::model::JumpType::DcAlFine => "D.C. al Fine",
            crate::model::JumpType::DcAlCoda => "D.C. al Coda",
            crate::model::JumpType::DsAlCoda => "D.S. al Coda",
            crate::model::JumpType::DsAlFine => "D.S. al Fine",
            crate::model::JumpType::Ds => "D.S.",
            _ => "",
        }
    };

    let is_da_capo = jump.kind.is_da_capo()
        || (jump.kind == crate::model::JumpType::User && jump.jump_to == "start");
    let is_dal_segno = jump.kind.is_dal_segno()
        || (jump.kind == crate::model::JumpType::User && jump.jump_to != "start");

    let sound_attributes: Vec<(&str, &str)> = if is_da_capo {
        vec![("dacapo", "yes")]
    } else if is_dal_segno {
        let target = if jump.text.is_empty() {
            "1"
        } else {
            jump.jump_to.as_str()
        };
        vec![("dalsegno", target)]
    } else {
        Vec::new()
    };

    if words.is_empty() {
        if !sound_attributes.is_empty() {
            sound(out, &sound_attributes);
        }
        return;
    }

    out.xml.start_element(
        "direction",
        &[("placement", placement_name(jump.placement))],
    );
    out.xml.start_element("direction-type", &[]);
    out.xml.tag("words", words);
    out.xml.end_element();
    if !sound_attributes.is_empty() {
        sound(out, &sound_attributes);
    }
    out.xml.end_element();
}

/// Vrai si un marqueur de la mesure appartient à la partie 0.
fn belongs_to_first_part(mark_track: Track, context: &Context<'_>) -> bool {
    let Some(track) = first_track(context, 0) else {
        return false;
    };
    mark_track == track
}

/// Écrit les marqueurs de début de mesure (`repeatAtMeasureStart`).
pub(super) fn repeat_at_measure_start(
    context: &Context<'_>,
    out: &mut Out,
    measure: &Measure,
    part_index: usize,
) {
    // « MuseScore limitation: repeats are always in the first part ».
    if part_index != 0 {
        return;
    }
    let jumps = jumps(context.score);
    for mark in &measure.repeat_marks {
        let RepeatMark::Marker(marker) = mark else {
            continue;
        };
        if !belongs_to_first_part(marker.track, context) {
            continue;
        }
        let kind = effective_marker_type(marker, &jumps);
        if kind.is_start_marker() {
            out.attr(false);
            direction_marker(out, marker, kind, &jumps);
        }
    }
}

/// Écrit marqueurs de fin et sauts (`repeatAtMeasureStop`).
pub(super) fn repeat_at_measure_stop(
    context: &Context<'_>,
    out: &mut Out,
    measure: &Measure,
    part_index: usize,
) {
    if part_index != 0 {
        return;
    }
    let jumps = jumps(context.score);
    for mark in &measure.repeat_marks {
        match mark {
            RepeatMark::Marker(marker) => {
                if !belongs_to_first_part(marker.track, context) {
                    continue;
                }
                let kind = effective_marker_type(marker, &jumps);
                if kind.is_stop_marker() {
                    out.attr(false);
                    direction_marker(out, marker, kind, &jumps);
                }
            }
            RepeatMark::Jump(jump) => {
                if !belongs_to_first_part(jump.track, context) {
                    continue;
                }
                out.attr(false);
                direction_jump(out, jump);
            }
        }
    }
}
