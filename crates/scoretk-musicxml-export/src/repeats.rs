// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

use scoretk_core::model::{
    Annotation, BarLine, BarLineType, Fermata, Jump, Marker, MarkerType, Measure, Placement,
    RepeatMark, Score, SegmentItem, Spanner, SpannerType, StaffId, Track,
};

use super::exportmusicxml::{Context, Out};
use super::notations;

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

/// Écrit `<measure-style><measure-repeat>` (`measureRepeat`).
///
/// Une mesure répétée ouvre le groupe quand la précédente ne le faisait pas
/// (ou avec un autre nombre de mesures) ; la première mesure qui n'est plus
/// répétée ferme le groupe. Le groupe est propre à une portée de la partie,
/// d'où l'attribut `number` quand la partie en a plusieurs.
pub(super) fn measure_style(
    context: &Context<'_>,
    out: &mut Out,
    measure: &Measure,
    measure_index: usize,
    part_index: usize,
) {
    let Some(part) = context.score.parts.get(part_index) else {
        return;
    };
    let previous = measure_index
        .checked_sub(1)
        .and_then(|index| context.score.measures.get(index))
        .and_then(|previous| previous.measure_repeat);
    let Some(mark) = measure.measure_repeat else {
        // Sortie d'un groupe : la mesure précédente répétait sur une portée de
        // cette partie. `measureRepeat` (exportmusicxml.cpp:6544-6571) construit
        // ses attributs dans la boucle de portée et utilise donc, pour le
        // `<measure-repeat type="stop">`, le même `number` que l'ouverture.
        // Le portage passait une position sentinelle 0, soit `number="0"`,
        // refusé par `xsd:staff-number` (xs:positiveInteger).
        if let Some(previous) = previous.filter(|mark| part.staves.contains(&mark.staff)) {
            if let Some(position) = part
                .staves
                .iter()
                .position(|staff| *staff == previous.staff)
            {
                write_measure_repeat(out, part.staves.len(), Some(position), None);
            }
        }
        return;
    };
    let Some(position) = part.staves.iter().position(|staff| *staff == mark.staff) else {
        return;
    };
    if previous.is_some_and(|previous| previous.staff == mark.staff && previous.count == mark.count)
    {
        return;
    }
    write_measure_repeat(out, part.staves.len(), Some(position), Some(mark.count));
}

/// Écrit un `<attributes><measure-style><measure-repeat>` (ou sa fin).
///
/// `position` est l'indice 0-basé de la portée dans la partie ; le numéro
/// MusicXML est `position + 1`, car `xsd:staff-number` est un entier positif.
fn write_measure_repeat(out: &mut Out, staves: usize, position: Option<usize>, count: Option<u16>) {
    out.attr(true);
    let number = position
        .filter(|_| staves > 1)
        .map(|position| (position + 1).to_string());
    let attributes: Vec<(&str, &str)> = number
        .as_deref()
        .map(|number| vec![("number", number)])
        .unwrap_or_default();
    out.xml.start_element("measure-style", &attributes);
    match count {
        Some(count) => {
            let count_text = count.to_string();
            if count > 1 {
                out.xml.tag_with(
                    "measure-repeat",
                    &[("slashes", &count_text), ("type", "start")],
                    &count_text,
                );
            } else {
                out.xml
                    .tag_with("measure-repeat", &[("type", "start")], &count_text);
            }
        }
        None => out.xml.tag_with("measure-repeat", &[("type", "stop")], ""),
    }
    out.xml.end_element();
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
/// Une mesure qui finit un système par un `<LayoutBreak>` explicite et dont la
/// suivante change d'armure ou de métrique reçoit en outre la barre de
/// courtoisie `light-light` que la mise en page de MuseScore ajoute à cet
/// endroit (`barlineRight` écrit alors le `BarLine` généré de type `DOUBLE`).
fn end_bar_line_type(
    context: &Context<'_>,
    measure: &Measure,
    measure_index: usize,
) -> BarLineType {
    if measure.end_bar_line == BarLineType::Normal
        && measure.line_break
        && next_measure_starts_with_signature(context.score, measure_index)
    {
        return BarLineType::Double;
    }
    if measure.end_bar_line == BarLineType::Normal
        && measure_index + 1 == context.score.measures.len()
        && end_bar_line(measure).is_none()
    {
        BarLineType::End
    } else {
        measure.end_bar_line
    }
}

/// Vrai si la mesure suivante ouvre un système par un changement d'armure ou
/// de métrique (`keysigTimesig` écrit alors `<key>`/`<time>`).
///
/// Un saut de section ne compte pas : MuseScore n'y ajoute pas de barre de
/// courtoisie (`keysig_78216`).
fn next_measure_starts_with_signature(score: &Score, measure_index: usize) -> bool {
    let Some(next) = score.measures.get(measure_index + 1) else {
        return false;
    };
    if next.section_break_before {
        return false;
    }
    next.segments.iter().any(|segment| {
        segment.tick == next.start_tick
            && segment
                .iter_elements()
                .any(|(_, item)| matches!(item, SegmentItem::KeySig(_) | SegmentItem::TimeSig(_)))
    })
}

/// Points d'orgue posés sur la barre de fin (`barlineHasFermata`).
///
/// Le lecteur range un point d'orgue situé à la fin de la mesure sur le
/// segment `EndBarLine` ; MuseScore ne l'exporte pas dans les notations de la
/// note mais dans `<barline location="right">`.
fn barline_fermatas<'a>(
    context: &'a Context<'a>,
    measure: &'a Measure,
    part_index: usize,
) -> Vec<&'a Fermata> {
    let Some(part) = context.score.parts.get(part_index) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for segment in &measure.segments {
        if segment.tick != measure.end_tick() {
            continue;
        }
        for entry in &segment.annotations {
            if !part.staves.contains(&entry.track.staff()) {
                continue;
            }
            if let Annotation::Fermata(fermata) = &entry.annotation {
                result.push(fermata);
            }
        }
    }
    result
}

/// Barre de fin portée par la mesure, si le `.mscx` en déclare une.
fn end_bar_line(measure: &Measure) -> Option<&BarLine> {
    let tick = measure.end_tick();
    measure
        .segments
        .iter()
        .filter(|segment| segment.kind == scoretk_core::model::SegmentType::EndBarLine)
        .find(|segment| segment.tick == tick)
        .and_then(|segment| {
            segment.iter_elements().find_map(|(_, item)| match item {
                scoretk_core::model::SegmentItem::BarLine(bar_line) => Some(bar_line),
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
    let fermatas = barline_fermatas(context, measure, part_index);

    if !need_style && volta.is_none() && special.is_none() && fermatas.is_empty() {
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
    for fermata in fermatas {
        notations::write_fermata(&mut out.xml, fermata);
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
            scoretk_core::model::JumpType::Dc => "D.C.",
            scoretk_core::model::JumpType::DcAlFine => "D.C. al Fine",
            scoretk_core::model::JumpType::DcAlCoda => "D.C. al Coda",
            scoretk_core::model::JumpType::DsAlCoda => "D.S. al Coda",
            scoretk_core::model::JumpType::DsAlFine => "D.S. al Fine",
            scoretk_core::model::JumpType::Ds => "D.S.",
            _ => "",
        }
    };

    let is_da_capo = jump.kind.is_da_capo()
        || (jump.kind == scoretk_core::model::JumpType::User && jump.jump_to == "start");
    let is_dal_segno = jump.kind.is_dal_segno()
        || (jump.kind == scoretk_core::model::JumpType::User && jump.jump_to != "start");

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

#[cfg(test)]
mod tests {
    use scoretk_core::model::{
        BarLine, BarLineType, ChordRest, Fraction, Jump, Marker, MarkerType, Measure, Part,
        Placement, RepeatMark, Score, Segment, SegmentItem, SegmentType, Spanner, SpannerAnchor,
        SpannerType, Track, VoltaInfo,
    };

    use crate::export_musicxml;

    fn portee(score: &mut Score) -> Track {
        let part = score.add_part(Part::new("Piano", "Pno."));
        let staff = score.add_staff(part);
        Track::new(staff, 0)
    }

    fn ajouter_mesure(score: &mut Score, number: usize, track: Track) -> usize {
        let start = score
            .measures
            .last()
            .map_or(Fraction::ZERO, Measure::end_tick);
        let mut measure = Measure::new(number, start, Fraction::new(4, 4));
        let mut segment = Segment::new(SegmentType::ChordRest, measure.start_tick, 1);
        segment.set_element(
            track,
            SegmentItem::ChordRest(ChordRest::full_measure_rest()),
        );
        measure.add_segment(segment);
        score.add_measure(measure).index()
    }

    fn marqueur(kind: MarkerType, label: &str, text: &str, track: Track) -> RepeatMark {
        RepeatMark::Marker(Marker {
            kind,
            label: label.to_owned(),
            text: text.to_owned(),
            placement: Placement::Above,
            track,
        })
    }

    #[test]
    fn reprise_avec_volta_et_fin_de_mesure() {
        let mut score = Score::new();
        let track = portee(&mut score);
        let first = ajouter_mesure(&mut score, 0, track);
        let last = ajouter_mesure(&mut score, 1, track);

        score.measures[first].start_bar_line = BarLineType::StartRepeat;
        score.measures[first].repeat_marks = vec![
            marqueur(MarkerType::Segno, "segno", "segno", track),
            marqueur(MarkerType::ToCoda, "coda", "To Coda", track),
        ];
        score.measures[last].end_bar_line = BarLineType::EndRepeat;
        score.measures[last].repeat_count = 3;
        score.measures[last].repeat_marks = vec![RepeatMark::Jump(Jump::new(
            "D.C. al Fine",
            "start",
            "fine",
            "",
            Placement::Above,
            track,
        ))];

        let mut volta = Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(score.measures[first].start_tick, track),
            SpannerAnchor::new(score.measures[last].end_tick(), track),
        );
        volta.text = Some("1, 2".to_owned());
        volta.volta = Some(VoltaInfo {
            endings: vec![1, 2],
            closed: false,
        });
        score.add_spanner(volta);

        let xml = export_musicxml(&score).expect("export");

        assert!(xml.contains("<barline location=\"left\">"));
        assert!(xml.contains("<bar-style>heavy-light</bar-style>"));
        assert!(xml.contains("<ending number=\"1, 2\" type=\"start\">1, 2</ending>"));
        assert!(xml.contains("<repeat direction=\"forward\"/>"));
        assert!(xml.contains("<segno/>"));
        assert!(xml.contains("<sound segno=\"segno\"/>"));
        assert!(xml.contains("<words>To Coda</words>"));
        assert!(xml.contains("<sound tocoda=\"1\"/>"));
        assert!(xml.contains("<words>D.C. al Fine</words>"));
        assert!(xml.contains("<sound dacapo=\"yes\"/>"));
        assert!(xml.contains("<ending number=\"1, 2\" type=\"discontinue\"/>"));
        assert!(xml.contains("<repeat direction=\"backward\" times=\"3\"/>"));
        assert!(xml.contains("<bar-style>light-heavy</bar-style>"));
    }

    #[test]
    fn barre_courte_au_milieu_et_volta_fermee() {
        let mut score = Score::new();
        let track = portee(&mut score);
        let first = ajouter_mesure(&mut score, 0, track);
        let last = ajouter_mesure(&mut score, 1, track);

        let mut bar_line = BarLine::new(BarLineType::Normal);
        bar_line.span_from = 2;
        bar_line.span_to = -2;
        let tick = score.measures[first].start_tick + Fraction::new(1, 2);
        let mut segment = Segment::new(SegmentType::BarLine, tick, 1);
        segment.set_element(track, SegmentItem::BarLine(bar_line));
        score.measures[first].add_segment(segment);

        score.measures[last].end_bar_line = BarLineType::EndRepeat;
        score.measures[last].repeat_count = 2;
        let mut volta = Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(score.measures[first].start_tick, track),
            SpannerAnchor::new(score.measures[last].end_tick(), track),
        );
        volta.text = Some("1.".to_owned());
        volta.volta = Some(VoltaInfo {
            endings: vec![1],
            closed: true,
        });
        score.add_spanner(volta);

        let xml = export_musicxml(&score).expect("export");
        assert!(xml.contains("<barline location=\"middle\">"));
        assert!(xml.contains("<bar-style>short</bar-style>"));
        assert!(xml.contains("<ending number=\"1\" type=\"stop\"/>"));
        // `times` n'est écrit que pour plus de deux lectures.
        assert!(xml.contains("<repeat direction=\"backward\"/>"));
        assert!(!xml.contains("times="));
    }

    #[test]
    fn derniere_mesure_sans_barre_explicite() {
        let mut score = Score::new();
        let track = portee(&mut score);
        ajouter_mesure(&mut score, 0, track);
        let xml = export_musicxml(&score).expect("export");
        assert!(xml.contains("<barline location=\"right\">"));
        assert!(xml.contains("<bar-style>light-heavy</bar-style>"));
    }
}
