// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`
// (`MusicXmlParserPass2::notations`, `fermata`, `arpeggio`, `glissando`,
// `slur`, `trill`) de MuseScore Studio, Copyright (C) MuseScore Limited and
// others, modifié (réécrit en Rust) en 2026.
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

//! Notations des notes : articulations, ornements, points d'orgue, arpèges,
//! liaisons d'expression, glissandos, trilles et doigtés.
//!
//! L'import de MuseScore crée des objets distincts (articulations, ornements,
//! spanners) ; le modèle les range sur l'accord, la note ou le segment. Les
//! extrémités des spanners sont des positions, ce qui évite les pointeurs.

use std::collections::BTreeMap;

use scoretk_core::model::{
    Accidental, AccidentalType, Annotation, Arpeggio, ArpeggioType, Articulation,
    ArticulationAnchor, ArticulationType, Breath, BreathType, Chord, Fermata, FermataType,
    Fingering, Glissando, GlissandoType, LineStyle, NoteId, Ornament, OrnamentAccidental,
    OrnamentType, Placement, Score, SegmentType, Spanner, SpannerAnchor, SpannerNotation,
    SpannerType, Track, Trill, TrillType,
};

use crate::pass2;
use crate::xml::Element;

/// État d'appariement des spanners notés sur les accords.
#[derive(Debug, Default)]
pub(crate) struct NotationState {
    slurs: BTreeMap<usize, usize>,
    glissandi: BTreeMap<(usize, bool), usize>,
    trills: BTreeMap<usize, usize>,
}

/// Notations lues sur une note.
#[derive(Debug, Default)]
pub(crate) struct ParsedNotations {
    articulations: Vec<Articulation>,
    ornaments: Vec<Ornament>,
    fermata: Option<Fermata>,
    arpeggio: Option<Arpeggio>,
    chord_line: Option<scoretk_core::model::ChordLineType>,
    fingerings: Vec<Fingering>,
    slurs: Vec<(usize, bool, Option<Placement>, bool)>,
    glissandi: Vec<(usize, bool, Glissando)>,
    trill: Option<(usize, Trill, bool, bool)>,
    trill_mark: bool,
    breath: Option<Breath>,
    visible: bool,
}

/// Lit les `<notations>` d'une note.
pub(crate) fn parse(element: &Element) -> ParsedNotations {
    let mut parsed = ParsedNotations::default();
    for notations in element.children_named("notations") {
        let visible = notations.attribute("print-object") != Some("no");
        parsed.visible = visible;
        for child in &notations.children {
            match child.name.as_str() {
                "articulations" => parse_articulations(child, visible, &mut parsed),
                "ornaments" => parse_ornaments(child, visible, &mut parsed),
                "fermata" => {
                    let mut value = fermata(child);
                    value.visible = value.visible && visible;
                    parsed.fermata = Some(value);
                }
                "arpeggiate" => {
                    let mut arpeggio = Arpeggio::new(match child.attribute("direction") {
                        Some("up") => ArpeggioType::Up,
                        Some("down") => ArpeggioType::Down,
                        _ => ArpeggioType::Normal,
                    });
                    arpeggio.visible = visible && child.attribute("print-object") != Some("no");
                    parsed.arpeggio = Some(arpeggio);
                }
                "non-arpeggiate" => {
                    let mut arpeggio = Arpeggio::new(ArpeggioType::Bracket);
                    arpeggio.visible = visible && child.attribute("print-object") != Some("no");
                    parsed.arpeggio = Some(arpeggio);
                }
                "technical" => {
                    for fingering in child.children_named("fingering") {
                        let mut value = Fingering::new(fingering.text_trimmed());
                        value.placement = placement(fingering.attribute("placement"));
                        value.visible =
                            visible && fingering.attribute("print-object") != Some("no");
                        parsed.fingerings.push(value);
                    }
                }
                "slur" => {
                    let number = number(child);
                    let place = placement(child.attribute("placement")).or_else(|| {
                        match child.attribute("orientation") {
                            Some("over") => Some(Placement::Above),
                            Some("under") => Some(Placement::Below),
                            _ => None,
                        }
                    });
                    match child.attribute("type") {
                        Some("start") => parsed.slurs.push((number, true, place, visible)),
                        Some("stop") => parsed.slurs.push((number, false, place, visible)),
                        _ => {}
                    }
                }
                "slide" | "glissando" => {
                    let number = number(child);
                    let wavy = child.name == "glissando";
                    let style = match child.attribute("line-type") {
                        Some("dashed") => LineStyle::Dashed,
                        Some("dotted") => LineStyle::Dotted,
                        _ => LineStyle::Solid,
                    };
                    let mut glissando = Glissando::new(if wavy {
                        GlissandoType::Wavy
                    } else {
                        GlissandoType::Straight(style)
                    });
                    glissando.text = child.text_trimmed().to_owned();
                    glissando.show_text = !glissando.text.is_empty();
                    glissando.visible = visible && child.attribute("print-object") != Some("no");
                    match child.attribute("type") {
                        Some("start") => parsed.glissandi.push((number, true, glissando)),
                        Some("stop") => {
                            let mut value = glissando;
                            value.text.clear();
                            value.show_text = false;
                            parsed.glissandi.push((number, false, value));
                        }
                        _ => {}
                    }
                }
                "wavy-line" => {
                    let number = number(child);
                    let start = child.attribute("type") == Some("start");
                    let stop = child.attribute("type") == Some("stop");
                    if start {
                        let kind = TrillType::TrillLine;
                        let mut trill = Trill::new(kind);
                        trill.accidental = notations
                            .children_named("accidental-mark")
                            .next()
                            .and_then(accidental_mark);
                        if let Some((_, _, _, existing)) = parsed.trill.as_mut() {
                            *existing = *existing || stop;
                        } else {
                            parsed.trill = Some((number, trill, true, stop));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if parsed.trill.is_none() && parsed.trill_mark {
        parsed
            .ornaments
            .push(Ornament::new(OrnamentType::TrillMark));
    }
    parsed
}

/// Applique les notations qui vivent sur l'accord.
pub(crate) fn apply_chord(parsed: &ParsedNotations, chord: &mut Chord) {
    chord.articulations.extend(parsed.articulations.clone());
    // Un trille en spanner écrit lui-même `trill-mark` : ne pas doubler
    // l'ornement quand le spanner existe.
    let mut ornaments = parsed.ornaments.clone();
    if parsed.trill.is_some() {
        ornaments.retain(|ornament| ornament.kind != OrnamentType::TrillMark);
    }
    chord.ornaments.extend(ornaments);
    if parsed.arpeggio.is_some() {
        chord.arpeggio = parsed.arpeggio;
    }
    if parsed.chord_line.is_some() {
        chord.chord_line = parsed.chord_line;
    }
}

/// Applique les notations attachées à la note et au segment.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_score(
    score: &mut Score,
    parsed: &ParsedNotations,
    note: Option<NoteId>,
    measure_index: usize,
    tick: scoretk_core::model::Fraction,
    end_tick: scoretk_core::model::Fraction,
    track: Track,
    state: &mut NotationState,
) {
    if let Some(note) = note {
        score.notes[note.index()]
            .fingerings
            .extend(parsed.fingerings.clone());
    }

    for (number, start, place, visible) in &parsed.slurs {
        let key = *number;
        if *start {
            let anchor = SpannerAnchor::new(tick, track);
            let mut spanner = Spanner::new(SpannerType::Slur, anchor, anchor);
            let mut slur = scoretk_core::model::Slur::new();
            slur.placement = *place;
            slur.visible = *visible;
            spanner.notation = Some(SpannerNotation::Slur(slur));
            state.slurs.insert(key, score.spanners.len());
            score.spanners.push(spanner);
        } else if let Some(index) = state.slurs.remove(&key) {
            if let Some(spanner) = score.spanners.get_mut(index) {
                spanner.end = SpannerAnchor::new(tick, track);
            }
        }
    }

    for (number, start, glissando) in &parsed.glissandi {
        let wavy = matches!(glissando.kind, GlissandoType::Wavy);
        let key = (*number, wavy);
        if *start {
            let anchor = SpannerAnchor::new(tick, track);
            let mut spanner = Spanner::new(SpannerType::Glissando, anchor, anchor);
            spanner.notation = Some(SpannerNotation::Glissando(glissando.clone()));
            state.glissandi.insert(key, score.spanners.len());
            score.spanners.push(spanner);
        } else if let Some(index) = state.glissandi.remove(&key) {
            if let Some(spanner) = score.spanners.get_mut(index) {
                spanner.end = SpannerAnchor::new(tick, track);
            }
        } else {
            // Arrêt sans départ : spanner d'un seul tick, l'export écrira les
            // deux moitiés au même endroit.
            let anchor = SpannerAnchor::new(tick, track);
            let mut spanner = Spanner::new(SpannerType::Glissando, anchor, anchor);
            spanner.notation = Some(SpannerNotation::Glissando(glissando.clone()));
            score.spanners.push(spanner);
        }
    }

    if let Some((number, trill, start, stop)) = &parsed.trill {
        let key = *number;
        if *start {
            let anchor = SpannerAnchor::new(tick, track);
            let mut spanner = Spanner::new(SpannerType::Trill, anchor, anchor);
            spanner.notation = Some(SpannerNotation::Trill(trill.clone()));
            state.trills.insert(key, score.spanners.len());
            score.spanners.push(spanner);
            if *stop {
                state.trills.remove(&key);
            }
        } else if let Some(index) = state.trills.remove(&key) {
            if let Some(spanner) = score.spanners.get_mut(index) {
                spanner.end = SpannerAnchor::new(tick, track);
            }
        }
    }

    if let Some(breath) = parsed.breath {
        if end_tick > tick {
            let index = pass2::get_segment(score, measure_index, SegmentType::Breath, end_tick);
            score.measures[measure_index].segments[index]
                .set_element(track, scoretk_core::model::SegmentItem::Breath(breath));
        }
    }

    if let Some(fermata) = parsed.fermata {
        let index = pass2::get_segment(score, measure_index, SegmentType::ChordRest, tick);
        score.measures[measure_index].segments[index]
            .add_annotation(track, Annotation::Fermata(fermata));
    }
}

/// Numéro d'un spanner MusicXML (base zéro).
fn number(element: &Element) -> usize {
    element
        .attribute("number")
        .and_then(|number| number.parse::<usize>().ok())
        .map_or(0, |number| number.saturating_sub(1))
}

/// Placement depuis un attribut MusicXML.
fn placement(value: Option<&str>) -> Option<Placement> {
    match value {
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => None,
    }
}

/// Ancre d'une articulation (`ArticulationAnchor`).
fn anchor(element: &Element) -> ArticulationAnchor {
    match element.attribute("placement") {
        Some("above") => ArticulationAnchor::Up,
        Some("below") => ArticulationAnchor::Down,
        _ => ArticulationAnchor::Auto,
    }
}

/// Articulations d'un `<articulations>`.
fn parse_articulations(element: &Element, notations_visible: bool, parsed: &mut ParsedNotations) {
    let visible = notations_visible && element.attribute("print-object") != Some("no");
    for child in &element.children {
        let kind = match child.name.as_str() {
            "staccato" => ArticulationType::Staccato,
            "staccatissimo" => ArticulationType::Staccatissimo,
            "accent" => ArticulationType::Accent,
            "strong-accent" => ArticulationType::Marcato,
            "tenuto" => ArticulationType::Tenuto,
            "detached-legato" => ArticulationType::Portato,
            "soft-accent" => ArticulationType::SoftAccent,
            "stress" => ArticulationType::Stress,
            "unstress" => ArticulationType::Unstress,
            "spiccato" => ArticulationType::Spiccato,
            "caesura" => {
                let kind = match child.text_trimmed() {
                    "curved" => BreathType::CaesuraCurved,
                    "short" => BreathType::CaesuraShort,
                    "thick" => BreathType::CaesuraThick,
                    "single" => BreathType::CaesuraSingle,
                    _ => BreathType::Caesura,
                };
                parsed.breath = Some(Breath {
                    kind,
                    placement: if child.attribute("placement") == Some("below") {
                        Placement::Below
                    } else {
                        Placement::Above
                    },
                });
                continue;
            }
            "breath-mark" => {
                let kind = match child.text_trimmed() {
                    "tick" => BreathType::Tick,
                    "upbow" => BreathType::Upbow,
                    _ => BreathType::Comma,
                };
                parsed.breath = Some(Breath {
                    kind,
                    placement: if child.attribute("placement") == Some("below") {
                        Placement::Below
                    } else {
                        Placement::Above
                    },
                });
                continue;
            }
            "falloff" => {
                parsed.chord_line = Some(scoretk_core::model::ChordLineType::Fall);
                continue;
            }
            "doit" => {
                parsed.chord_line = Some(scoretk_core::model::ChordLineType::Doit);
                continue;
            }
            "plop" => {
                parsed.chord_line = Some(scoretk_core::model::ChordLineType::Plop);
                continue;
            }
            "scoop" => {
                parsed.chord_line = Some(scoretk_core::model::ChordLineType::Scoop);
                continue;
            }
            "other-articulation" => {
                ArticulationType::Other(child.attribute("smufl").unwrap_or_default().to_owned())
            }
            _ => continue,
        };
        let mut articulation = Articulation::new(kind);
        articulation.anchor = anchor(child);
        articulation.visible = visible;
        if child.name == "strong-accent" {
            articulation.up = child.attribute("type") != Some("down");
        }
        parsed.articulations.push(articulation);
    }
}

/// Ornements d'un `<ornaments>`.
fn parse_ornaments(element: &Element, notations_visible: bool, parsed: &mut ParsedNotations) {
    let visible = notations_visible && element.attribute("print-object") != Some("no");
    let mut accidentals = Vec::new();
    for child in element.children_named("accidental-mark") {
        if let Some(accidental) = accidental_mark(child) {
            accidentals.push(OrnamentAccidental {
                accidental,
                placement: placement(child.attribute("placement")).unwrap_or(Placement::Above),
            });
        }
    }
    for child in &element.children {
        let kind = match child.name.as_str() {
            "trill-mark" => {
                parsed.trill_mark = true;
                continue;
            }
            "turn" => {
                if child.attribute("slash") == Some("yes") {
                    OrnamentType::TurnSlash
                } else {
                    OrnamentType::Turn
                }
            }
            "inverted-turn" => OrnamentType::InvertedTurn,
            "vertical-turn" => OrnamentType::VerticalTurn,
            "inverted-vertical-turn" => OrnamentType::InvertedVerticalTurn,
            "mordent" => match (child.attribute("long"), child.attribute("approach")) {
                (Some("yes"), Some("below")) => OrnamentType::MordentLongApproachBelow,
                (Some("yes"), Some("above")) => OrnamentType::MordentLongApproachAbove,
                (Some("yes"), _) => OrnamentType::MordentLong,
                _ => OrnamentType::Mordent,
            },
            "inverted-mordent" => {
                match (
                    child.attribute("long"),
                    child.attribute("approach"),
                    child.attribute("departure"),
                ) {
                    (Some("yes"), Some("below"), _) => {
                        OrnamentType::InvertedMordentLongApproachBelow
                    }
                    (Some("yes"), Some("above"), _) => {
                        OrnamentType::InvertedMordentLongApproachAbove
                    }
                    (Some("yes"), _, Some("below")) => {
                        OrnamentType::InvertedMordentLongDepartureBelow
                    }
                    (Some("yes"), _, Some("above")) => {
                        OrnamentType::InvertedMordentLongDepartureAbove
                    }
                    (Some("yes"), _, _) => OrnamentType::InvertedMordentLong,
                    _ => OrnamentType::InvertedMordent,
                }
            }
            "haydn" => OrnamentType::Haydn,
            "schleifer" => OrnamentType::Schleifer,
            "other-ornament" => {
                OrnamentType::Other(child.attribute("smufl").unwrap_or_default().to_owned())
            }
            "wavy-line" => {
                let number = number(child);
                let start = child.attribute("type") == Some("start");
                let stop = child.attribute("type") == Some("stop");
                if start {
                    let mut trill = Trill::new(TrillType::TrillLine);
                    trill.accidental = accidentals.first().map(|entry| entry.accidental);
                    if let Some((_, _, _, existing)) = parsed.trill.as_mut() {
                        *existing = *existing || stop;
                    } else {
                        parsed.trill = Some((number, trill, true, stop));
                    }
                }
                continue;
            }
            "accidental-mark" | "tremolo" => continue,
            _ => continue,
        };
        let mut ornament = Ornament::new(kind);
        ornament.placement = placement(child.attribute("placement"));
        ornament.accidentals = accidentals.clone();
        ornament.visible = visible;
        parsed.ornaments.push(ornament);
    }
}

/// Point d'orgue (`fermata`).
fn fermata(element: &Element) -> Fermata {
    let kind = match element.text_trimmed() {
        "angled" => FermataType::Short,
        "square" => FermataType::Long,
        "double-angled" => FermataType::VeryShort,
        "double-square" => FermataType::VeryLong,
        "double-dot" => FermataType::LongHenze,
        "half-curve" => FermataType::ShortHenze,
        "curlew" => FermataType::Curlew,
        _ => FermataType::Normal,
    };
    let mut fermata = Fermata::new(kind);
    fermata.placement = if element.attribute("type") == Some("inverted") {
        Placement::Below
    } else {
        Placement::Above
    };
    fermata.visible = element.attribute("print-object") != Some("no");
    fermata
}

/// Altération d'un `<accidental-mark>`.
fn accidental_mark(element: &Element) -> Option<Accidental> {
    let kind = match element.text_trimmed() {
        "flat" => AccidentalType::Flat,
        "natural" => AccidentalType::Natural,
        "sharp" => AccidentalType::Sharp,
        "flat-flat" => AccidentalType::DoubleFlat,
        "double-sharp" | "sharp-sharp" => AccidentalType::DoubleSharp,
        "natural-flat" => AccidentalType::NaturalFlat,
        "natural-sharp" => AccidentalType::NaturalSharp,
        "triple-flat" => AccidentalType::TripleFlat,
        "triple-sharp" => AccidentalType::TripleSharp,
        _ => return None,
    };
    let mut accidental = Accidental::new(kind);
    accidental.parentheses = element.attribute("parentheses") == Some("yes");
    accidental.bracket = element.attribute("bracket") == Some("yes");
    accidental.visible = element.attribute("print-object") != Some("no");
    Some(accidental)
}
