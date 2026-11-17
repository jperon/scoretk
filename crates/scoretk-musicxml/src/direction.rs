// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/importmusicxmlpass2.cpp`
// (`MusicXmlParserDirection` : `direction`, `directionType`, `sound`,
// `dynamics`, `metronome`, `wedge`, `octave-shift`, `pedal`, `dashes`,
// `bracket`, `handleRepeats`) de MuseScore Studio, Copyright (C) MuseScore
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

//! Directions MusicXML : nuances, soufflets, textes, tempos et reprises.
//!
//! Portage de `MusicXmlParserDirection` (`importmusicxmlpass2.cpp`) réduit à
//! ce que le modèle de `mscz2xml` sait représenter : [`Annotation`] pour les
//! nuances, tempos, textes et repères, [`Spanner`] pour les soufflets,
//! ottavas, pédales et lignes de texte, [`RepeatMark`] pour les segnos, codas,
//! fines et sauts.
//!
//! Les annotations sont rattachées au segment `ChordRest` du tick de la
//! direction, comme `addElemOffset` dans le C++.

use std::collections::BTreeMap;

use mscz2xml::model::{
    Annotation, Dynamic, Fraction, Hairpin, HairpinType, HookType, Instrument, Jump, Marker,
    MarkerType, OttavaType, PedalInfo, Placement, RehearsalMark, RepeatMark, Score, Spanner,
    SpannerAnchor, SpannerType, SystemText, Tempo, TextLineInfo, TextLineStyle, Track, Transpose,
};

use crate::pass1::Analysis;
use crate::pass2;
use crate::xml::Element;

/// Lit un `<sound><instrument-change>` placé directement dans la mesure.
pub(crate) fn read_standalone_instrument_change(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    change: &Element,
    relative_tick: Fraction,
    state: &mut DirectionState,
) {
    read_instrument_change(
        score,
        analysis,
        part_index,
        measure_index,
        change,
        relative_tick,
        None,
        state,
    );
}

/// Crée l'annotation d'un changement d'instrument resté sans texte.
pub(crate) fn flush_pending_instrument(
    score: &mut Score,
    part_index: usize,
    measure_index: usize,
    state: &mut DirectionState,
) {
    let Some((tick, instrument, placement)) = state.pending_instrument.take() else {
        return;
    };
    let measure = &score.measures[measure_index];
    if tick < measure.start_tick || tick >= measure.end_tick() {
        state.pending_instrument = Some((tick, instrument, placement));
        return;
    }
    let Some(staff_id) = score.parts[part_index].staves.first().copied() else {
        return;
    };
    let track = Track::new(staff_id, 0);
    let mut change = mscz2xml::model::InstrumentChange::new(instrument.track_name.clone());
    change.placement = placement;
    change.visible = false;
    change.instrument = Some(instrument);
    add_annotation(
        score,
        measure_index,
        tick,
        track,
        Annotation::InstrumentChange(change),
    );
}

/// Type de spanner en cours, pour apparier départ et arrêt par numéro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SpannerTag {
    Wedge,
    Ottava,
    Pedal,
    Dashes,
    Bracket,
}

/// État conservé pendant la lecture d'une partie.
#[derive(Debug, Default)]
pub(crate) struct DirectionState {
    /// Spanners ouverts, par type et numéro MusicXML (base zéro).
    open: BTreeMap<(SpannerTag, usize), usize>,
    /// Changement d'instrument en attente de son texte (`<direction><words>`).
    pub(crate) pending_instrument: Option<(Fraction, Instrument, Placement)>,
    /// Dernière transposition lue, appliquée au prochain changement
    /// d'instrument (`<transpose>` précède `<sound>` dans le fichier).
    pub(crate) last_transpose: Option<Transpose>,
    /// Ottavas actives par (portée, numéro) : décalage écrit (+1 pour `8va`).
    active_ottava: BTreeMap<(usize, usize), i32>,
    /// Liaisons, glissandos et trilles en cours.
    pub(crate) notations: crate::notations::NotationState,
}

impl DirectionState {
    /// Réinitialise les changements d'instrument au début d'une partie.
    pub(crate) fn start_part(&mut self) {
        self.open.clear();
        self.pending_instrument = None;
        self.last_transpose = None;
        self.active_ottava.clear();
    }

    /// Décalage d'octave écrit d'une portée à la position courante.
    ///
    /// L'import retire ce décalage de la hauteur lue : le modèle stocke la
    /// hauteur sonore, l'export réapplique l'ottava (`xmlSetPitch`).
    pub(crate) fn octave_shift(&self, staff: usize) -> i32 {
        self.active_ottava
            .iter()
            .filter(|((position, _), _)| *position == staff)
            .map(|(_, shift)| *shift)
            .sum()
    }
}

/// Lit un `<direction>` (`MusicXmlParserDirection::direction`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_direction(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    element: &Element,
    m_time: Fraction,
    state: &mut DirectionState,
) {
    let divisions = analysis.divisions.max(1);
    let placement_attribute = element.attribute("placement");
    let system = element.attribute("system") == Some("only-top");
    let visible = element.attribute("print-object") != Some("no");
    let mut offset = Fraction::ZERO;
    let mut staff_position = 0usize;

    let mut words = String::new();
    let mut words_markup = String::new();
    let mut rehearsal_text = String::new();
    let mut dynamics: Vec<(String, bool)> = Vec::new();
    let mut dynamic_placement: Option<Placement> = None;
    let mut metronome: Option<Metronome> = None;
    let mut starts: Vec<SpannerStart> = Vec::new();
    let mut stops: Vec<(SpannerTag, usize, bool)> = Vec::new();
    let mut sound_tempo: Option<f64> = None;
    let mut sound_dynamics: Option<f64> = None;
    let mut sound_coda: Option<String> = None;
    let mut sound_segno: Option<String> = None;
    let mut sound_to_coda: Option<String> = None;
    let mut sound_fine = false;
    let mut sound_da_capo = false;
    let mut sound_dal_segno: Option<String> = None;
    let mut symbol_words = String::new();
    let mut tbox_words: Option<(String, Option<f64>)> = None;
    let mut words_font: Option<mscz2xml::model::LineFont> = None;
    let mut line_stops: BTreeMap<(SpannerTag, usize), (HookType, f64)> = BTreeMap::new();
    let mut wedge_spread: Option<f64> = None;

    for child in &element.children {
        match child.name.as_str() {
            "direction-type" => {
                for kind in &child.children {
                    let number = kind
                        .attribute("number")
                        .and_then(|number| number.parse::<usize>().ok())
                        .map_or(0, |number| number.saturating_sub(1));
                    match kind.name.as_str() {
                        "dynamics" => {
                            dynamic_placement = placement(kind.attribute("placement"));
                            read_dynamics(kind, &mut dynamics);
                        }
                        "wedge" => {
                            if let Some(spread) = kind
                                .attribute("spread")
                                .and_then(|spread| spread.parse::<f64>().ok())
                            {
                                wedge_spread = Some(spread);
                            }
                            read_wedge(kind, number, &mut starts, &mut stops);
                        }
                        "octave-shift" => read_octave_shift(kind, number, &mut starts, &mut stops),
                        "pedal" => read_pedal(kind, number, &mut starts, &mut stops),
                        "dashes" => {
                            if let Some((hook, height)) = read_text_line(
                                kind,
                                number,
                                SpannerTag::Dashes,
                                &mut starts,
                                &mut stops,
                            ) {
                                line_stops.insert((SpannerTag::Dashes, number), (hook, height));
                            }
                        }
                        "bracket" => {
                            if let Some((hook, height)) = read_text_line(
                                kind,
                                number,
                                SpannerTag::Bracket,
                                &mut starts,
                                &mut stops,
                            ) {
                                line_stops.insert((SpannerTag::Bracket, number), (hook, height));
                            }
                        }
                        "words" => {
                            if kind.attribute("valign") == Some("top") {
                                let size = kind
                                    .attribute("font-size")
                                    .and_then(|size| size.parse::<f64>().ok());
                                tbox_words = Some((kind.text_trimmed().to_owned(), size));
                            } else {
                                words.push_str(kind.text_trimmed());
                                words_markup.push_str(&formatted_words(kind));
                                words_font = Some(line_font(kind));
                            }
                        }
                        "rehearsal" => {
                            rehearsal_text.push_str(kind.text_trimmed());
                        }
                        "metronome" => {
                            metronome = read_metronome(kind);
                        }
                        "segno" => {
                            symbol_words.push_str("<sym>segno</sym>");
                        }
                        "coda" => {
                            symbol_words.push_str("<sym>coda</sym>");
                        }
                        _ => {}
                    }
                }
            }
            "offset" => {
                if let Some(value) = child.text_trimmed().parse::<i64>().ok() {
                    offset = Fraction::from_ticks(value, divisions * 4);
                }
            }
            "sound" => {
                sound_tempo = child
                    .attribute("tempo")
                    .and_then(|tempo| tempo.parse::<f64>().ok());
                sound_dynamics = child
                    .attribute("dynamics")
                    .and_then(|value| value.parse::<f64>().ok());
                sound_coda = child.attribute("coda").map(str::to_owned);
                sound_segno = child.attribute("segno").map(str::to_owned);
                sound_to_coda = child.attribute("tocoda").map(str::to_owned);
                sound_fine = child.attribute("fine").is_some();
                sound_da_capo = child.attribute("dacapo").is_some();
                sound_dal_segno = child.attribute("dalsegno").map(str::to_owned);
                if let Some(change) = child.child("instrument-change") {
                    read_instrument_change(
                        score,
                        analysis,
                        part_index,
                        measure_index,
                        change,
                        m_time + offset,
                        placement_attribute,
                        state,
                    );
                }
            }
            "staff" => {
                staff_position = child
                    .text_trimmed()
                    .parse::<usize>()
                    .ok()
                    .map_or(0, |number| number.saturating_sub(1));
            }
            _ => {}
        }
    }

    let Some(staff_id) = score.parts[part_index].staves.get(staff_position).copied() else {
        return;
    };
    let track = Track::new(staff_id, 0);
    let measure_start = score.measures[measure_index].start_tick;
    let tick = measure_start + m_time + offset;
    let default_placement = placement(placement_attribute);

    if let Some(spread) = wedge_spread {
        score.style.hairpin_height_sp = spread / 10.0;
    }

    if let Some((text, font_size)) = tbox_words {
        if !text.is_empty() {
            let mut tbox =
                mscz2xml::model::TboxText::new(mscz2xml::model::RichText::new(text), measure_index);
            tbox.placement = default_placement.unwrap_or(Placement::Above);
            tbox.font_size = font_size;
            score.add_tbox_text(tbox);
        }
    }

    // Reprises : le C++ les traite avant les textes et vide `m_wordsText`.
    let repeat_consumed = read_repeats(
        score,
        measure_index,
        tick,
        &words,
        &symbol_words,
        default_placement,
        track,
        sound_coda.as_deref(),
        sound_segno.as_deref(),
        sound_to_coda.as_deref(),
        sound_fine,
        sound_da_capo,
        sound_dal_segno.as_deref(),
    );

    // Tempo : `<metronome>` et/ou `<sound tempo>`.
    let mut tempo_added = false;
    let metronome_tempo = metronome
        .as_ref()
        .and_then(|metronome| metronome.beats_per_second)
        .or(sound_tempo.map(|tempo| tempo / 60.0));
    if let Some(metronome) = &metronome {
        let mut tempo = Tempo::new(metronome.markup.clone());
        tempo.beats_per_second = metronome_tempo;
        tempo.placement = Placement::Above;
        tempo.visible = visible;
        add_annotation(score, measure_index, tick, track, Annotation::Tempo(tempo));
        tempo_added = true;
    } else if !words.is_empty() && sound_tempo.is_some() {
        let mut tempo = Tempo::new(words_markup.clone());
        tempo.beats_per_second = metronome_tempo;
        tempo.placement = Placement::Above;
        tempo.visible = visible;
        add_annotation(score, measure_index, tick, track, Annotation::Tempo(tempo));
        tempo_added = true;
    } else if let Some(tempo) = sound_tempo {
        // Direction sans texte mais avec `sound tempo` : tempo invisible
        // (voir `MusicXmlParserDirection::direction`).
        let mut annotation = Tempo::new(String::new());
        annotation.beats_per_second = Some(tempo / 60.0);
        annotation.visible = false;
        add_annotation(
            score,
            measure_index,
            tick,
            track,
            Annotation::Tempo(annotation),
        );
        tempo_added = true;
    }

    // Un texte de direction qui accompagne une ligne (`<dashes>`/`<bracket>`)
    // est le texte de début de cette ligne, pas un texte de portée.
    let mut words_for_line = false;
    let mut dynamics_for_hairpin = false;
    let mut words_for_hairpin = false;
    if !words.is_empty() {
        for start in starts.iter_mut() {
            if matches!(start.tag, SpannerTag::Dashes | SpannerTag::Bracket) {
                if let Some(info) = start.text_line.as_mut() {
                    info.system = system;
                    info.begin_text = Some(mscz2xml::model::RichText::new(words_markup.clone()));
                    words_for_line = true;
                }
            }
        }
    }
    for start in starts.iter_mut() {
        if start.tag != SpannerTag::Wedge {
            continue;
        }
        if let Some(hairpin) = start.hairpin.as_mut() {
            if !dynamics.is_empty() {
                hairpin.begin_text = dynamics
                    .iter()
                    .map(|(name, _)| dynamic_markup(name))
                    .collect::<String>();
                dynamics_for_hairpin = true;
            } else if !words.is_empty() && !words_for_line {
                hairpin.begin_text = words.clone();
                if let Some(font) = &words_font {
                    hairpin.begin_font = font.clone();
                }
                words_for_hairpin = true;
            }
        }
    }
    if stops.iter().any(|stop| stop.0 == SpannerTag::Wedge) {
        if !dynamics.is_empty() {
            dynamics_for_hairpin = true;
        } else if !words.is_empty() {
            words_for_hairpin = true;
        }
    }

    // Texte de portée, système ou repère de répétition.
    if !repeat_consumed && !tempo_added && !words_for_line && !words_for_hairpin {
        if !rehearsal_text.is_empty() {
            let mut mark = RehearsalMark::new(rehearsal_text);
            mark.placement = default_placement.unwrap_or(Placement::Above);
            add_annotation(
                score,
                measure_index,
                tick,
                track,
                Annotation::RehearsalMark(mark),
            );
        } else if !words.is_empty() {
            if let Some((change_tick, instrument, change_placement)) =
                state.pending_instrument.clone()
            {
                if change_tick == tick {
                    let mut change = mscz2xml::model::InstrumentChange::new(words_markup.clone());
                    change.placement = change_placement;
                    change.visible = visible;
                    change.instrument = Some(instrument);
                    add_annotation(
                        score,
                        measure_index,
                        tick,
                        track,
                        Annotation::InstrumentChange(change),
                    );
                    state.pending_instrument = None;
                }
            } else if system {
                let mut text = SystemText::new(words_markup.clone());
                text.placement = default_placement.unwrap_or(Placement::Above);
                text.visible = visible;
                add_annotation(
                    score,
                    measure_index,
                    tick,
                    track,
                    Annotation::SystemText(text),
                );
            } else {
                let mut text = mscz2xml::model::StaffText::new(words_markup.clone());
                text.placement = default_placement.unwrap_or(Placement::Above);
                text.visible = visible;
                add_annotation(
                    score,
                    measure_index,
                    tick,
                    track,
                    Annotation::StaffText(text),
                );
            }
        } else if !symbol_words.is_empty() {
            // Direction qui ne porte qu'un symbole (segno, coda) sans
            // `<sound>` : MuseScore en fait un texte de portée.
            let mut text = mscz2xml::model::StaffText::new(symbol_words.clone());
            text.placement = default_placement.unwrap_or(Placement::Above);
            text.visible = visible;
            add_annotation(
                score,
                measure_index,
                tick,
                track,
                Annotation::StaffText(text),
            );
        }
    }

    // Nuances.
    for (text, custom) in if dynamics_for_hairpin {
        Vec::new()
    } else {
        dynamics.clone()
    } {
        let mut dynamic = Dynamic::new(text);
        dynamic.custom = custom;
        dynamic.placement = dynamic_placement
            .or(default_placement)
            .unwrap_or(Placement::Below);
        dynamic.visible = visible;
        if let Some(percent) = sound_dynamics {
            dynamic.velocity = Some(((percent * 0.9).round() as i32).clamp(0, 127) as u8);
        }
        add_annotation(
            score,
            measure_index,
            tick,
            track,
            Annotation::Dynamic(dynamic),
        );
    }

    // Spanners.
    for stop in stops {
        if stop.0 == SpannerTag::Ottava {
            state.active_ottava.remove(&(staff_position, stop.1));
        }
        if let Some(index) = close_spanner(score, state, stop, tick, track) {
            if let Some((hook, height)) = line_stops.get(&(stop.0, stop.1)) {
                if let Some(info) = score.spanners[index].text_line.as_mut() {
                    info.end_hook = *hook;
                    info.end_hook_height = *height;
                    info.system = system;
                }
            }
            if stop.0 == SpannerTag::Wedge {
                if let Some(hairpin) = score.spanners[index].hairpin.as_mut() {
                    if !dynamics.is_empty() {
                        hairpin.end_text = dynamics
                            .iter()
                            .map(|(name, _)| dynamic_markup(name))
                            .collect::<String>();
                    } else if !words.is_empty() {
                        hairpin.end_text = words.clone();
                        if let Some(font) = &words_font {
                            hairpin.end_font = font.clone();
                        }
                    }
                }
            }
        }
    }
    for start in starts {
        if start.tag == SpannerTag::Ottava {
            let shift = match start.kind {
                SpannerType::Ottava(OttavaType::OctaveUp) => 1,
                SpannerType::Ottava(OttavaType::OctaveDown) => -1,
                SpannerType::Ottava(OttavaType::FifteenthUp) => 2,
                SpannerType::Ottava(OttavaType::FifteenthDown) => -2,
                _ => 0,
            };
            state
                .active_ottava
                .insert((staff_position, start.number), shift);
        }
        open_spanner(score, state, start, tick, track);
    }
}

/// Une annotation est rattachée au segment `ChordRest` du tick.
fn add_annotation(
    score: &mut Score,
    measure_index: usize,
    tick: Fraction,
    track: Track,
    annotation: Annotation,
) {
    let index = pass2::get_segment(
        score,
        measure_index,
        mscz2xml::model::SegmentType::ChordRest,
        tick,
    );
    score.measures[measure_index].segments[index].add_annotation(track, annotation);
}

/// Police d'un `<words>` (`LineFont`), pour les textes de soufflet.
fn line_font(element: &Element) -> mscz2xml::model::LineFont {
    let mut font = mscz2xml::model::LineFont::default();
    if let Some(family) = element.attribute("font-family") {
        font.family = family.to_owned();
    }
    if let Some(size) = element.attribute("font-size") {
        if let Ok(size) = size.parse::<f64>() {
            font.size = size;
        }
    }
    font.italic = element.attribute("font-style") == Some("italic");
    font.bold = element.attribute("font-weight") == Some("bold");
    font
}

/// Balisage MuseScore d'un `<words>` d'après ses attributs de format.
///
/// Le modèle de texte ne stocke que du balisage : on rouvre donc `<i>`,
/// `<b>`, `<u>` et `<font>` pour que l'export réécrive les mêmes attributs.
fn formatted_words(element: &Element) -> String {
    let text = element.text_trimmed();
    let mut open = String::new();
    let mut close = String::new();
    if element.attribute("font-style") == Some("italic") {
        open.push_str("<i>");
        close.insert_str(0, "</i>");
    }
    if element.attribute("font-weight") == Some("bold") {
        open.push_str("<b>");
        close.insert_str(0, "</b>");
    }
    if element.attribute("underline") == Some("1") {
        open.push_str("<u>");
        close.insert_str(0, "</u>");
    }
    if element.attribute("line-through") == Some("1") {
        open.push_str("<s>");
        close.insert_str(0, "</s>");
    }
    if let Some(family) = element.attribute("font-family") {
        open.push_str(&format!("<font face=\"{family}\">"));
        close.insert_str(0, "</font>");
    }
    if let Some(size) = element.attribute("font-size") {
        open.push_str(&format!("<font size=\"{size}\">"));
        close.insert_str(0, "</font>");
    }
    format!("{open}{text}{close}")
}

/// Balisage MuseScore d'une nuance, pour le texte d'un soufflet.
fn dynamic_markup(name: &str) -> String {
    let p = "<sym>dynamicPiano</sym>";
    let f = "<sym>dynamicForte</sym>";
    let m = "<sym>dynamicMezzo</sym>";
    let s = "<sym>dynamicSforzando</sym>";
    let z = "<sym>dynamicZ</sym>";
    match name {
        "pppppp" => p.repeat(6),
        "ppppp" => p.repeat(5),
        "pppp" => p.repeat(4),
        "ppp" => p.repeat(3),
        "pp" => p.repeat(2),
        "p" => p.to_owned(),
        "mp" => format!("{m}{p}"),
        "mf" => format!("{m}{f}"),
        "f" => f.to_owned(),
        "ff" => f.repeat(2),
        "fff" => f.repeat(3),
        "ffff" => f.repeat(4),
        "fp" => format!("{f}{p}"),
        "sfz" => format!("{s}{f}{z}"),
        "sf" => format!("{s}{f}"),
        other => other.to_owned(),
    }
}

/// Placement depuis un attribut MusicXML.
fn placement(value: Option<&str>) -> Option<Placement> {
    match value {
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => None,
    }
}

/// Nuances d'un `<dynamics>` (`parseDynamicsChildren`).
fn read_dynamics(element: &Element, result: &mut Vec<(String, bool)>) {
    for child in &element.children {
        if child.name == "other-dynamics" {
            let text = child.text_trimmed();
            if !text.is_empty() {
                result.push((text.to_owned(), true));
            }
        } else if !child.name.is_empty() {
            result.push((child.name.clone(), false));
        }
    }
}

/// Métronome lu dans `<metronome>`.
struct Metronome {
    /// Texte MuseScore de la formule (`<sym>metNoteQuarterUp</sym> = 120`).
    markup: String,
    /// Valeur en battements par seconde.
    beats_per_second: Option<f64>,
}

/// Lit `<metronome>` (`metronome`).
fn read_metronome(element: &Element) -> Option<Metronome> {
    let unit = element.child_text("beat-unit")?;
    let symbol = metronome_symbol(unit)?;
    let dots = element
        .children
        .iter()
        .filter(|child| child.name == "beat-unit-dot")
        .count();
    let mut markup = format!("<sym>{symbol}</sym>");
    for _ in 0..dots {
        markup.push_str("<sym>metAugmentationDot</sym>");
    }
    let mut beats_per_second = None;
    if let Some(per_minute) = element.child_text("per-minute") {
        if let Ok(value) = per_minute.trim().replace(',', ".").parse::<f64>() {
            markup.push_str(" = ");
            markup.push_str(per_minute.trim());
            beats_per_second = Some(value / 60.0);
        }
    }
    Some(Metronome {
        markup,
        beats_per_second,
    })
}

/// Nom SMuFL du symbole d'une figure de métronome.
fn metronome_symbol(unit: &str) -> Option<&'static str> {
    Some(match unit {
        "breve" => "metNoteDoubleWhole",
        "whole" => "metNoteWhole",
        "half" => "metNoteHalfUp",
        "quarter" => "metNoteQuarterUp",
        "eighth" => "metNote8thUp",
        "16th" => "metNote16thUp",
        "32nd" => "metNote32ndUp",
        "64th" => "metNote64thUp",
        _ => return None,
    })
}

/// Soufflet en cours de lecture.
struct SpannerStart {
    tag: SpannerTag,
    number: usize,
    kind: SpannerType,
    text_line: Option<TextLineInfo>,
    pedal: Option<PedalInfo>,
    hairpin: Option<Hairpin>,
}

/// Lit un `<wedge>`.
fn read_wedge(
    element: &Element,
    number: usize,
    starts: &mut Vec<SpannerStart>,
    stops: &mut Vec<(SpannerTag, usize, bool)>,
) {
    let kind = element.attribute("type").unwrap_or("");
    let placement = placement(element.attribute("placement")).unwrap_or(Placement::Below);
    match kind {
        "crescendo" | "diminuendo" => {
            let mut hairpin = Hairpin::new();
            hairpin.placement = placement;
            hairpin.niente = element.attribute("niente") == Some("yes");
            starts.push(SpannerStart {
                tag: SpannerTag::Wedge,
                number,
                kind: SpannerType::Hairpin(if kind == "crescendo" {
                    HairpinType::Crescendo
                } else {
                    HairpinType::Diminuendo
                }),
                text_line: None,
                pedal: None,
                hairpin: Some(hairpin),
            });
        }
        "stop" => stops.push((
            SpannerTag::Wedge,
            number,
            element.attribute("niente") == Some("yes"),
        )),
        _ => {}
    }
}

/// Lit un `<octave-shift>`.
fn read_octave_shift(
    element: &Element,
    number: usize,
    starts: &mut Vec<SpannerStart>,
    stops: &mut Vec<(SpannerTag, usize, bool)>,
) {
    let kind = element.attribute("type").unwrap_or("");
    let size = element.attribute("size");
    match kind {
        "down" => starts.push(ottava_start(
            number,
            if size == Some("15") {
                OttavaType::FifteenthUp
            } else {
                OttavaType::OctaveUp
            },
        )),
        "up" => starts.push(ottava_start(
            number,
            if size == Some("15") {
                OttavaType::FifteenthDown
            } else {
                OttavaType::OctaveDown
            },
        )),
        "stop" => stops.push((SpannerTag::Ottava, number, false)),
        _ => {}
    }
}

fn ottava_start(number: usize, kind: OttavaType) -> SpannerStart {
    SpannerStart {
        tag: SpannerTag::Ottava,
        number,
        kind: SpannerType::Ottava(kind),
        text_line: None,
        pedal: None,
        hairpin: None,
    }
}

/// Lit un `<pedal>`.
fn read_pedal(
    element: &Element,
    number: usize,
    starts: &mut Vec<SpannerStart>,
    stops: &mut Vec<(SpannerTag, usize, bool)>,
) {
    let kind = element.attribute("type").unwrap_or("");
    let placement = placement(element.attribute("placement")).unwrap_or(Placement::Below);
    match kind {
        "start" | "change" | "resume" | "sostenuto" => {
            let mut info = PedalInfo::new();
            info.placement = placement;
            info.line_visible = element.attribute("line") != Some("no");
            starts.push(SpannerStart {
                tag: SpannerTag::Pedal,
                number,
                kind: SpannerType::Pedal,
                text_line: None,
                pedal: Some(info),
                hairpin: None,
            });
        }
        "stop" => stops.push((SpannerTag::Pedal, number, true)),
        "discontinue" => stops.push((SpannerTag::Pedal, number, false)),
        _ => {}
    }
}

/// Lit un `<dashes>` ou un `<bracket>`.
fn read_text_line(
    element: &Element,
    number: usize,
    tag: SpannerTag,
    starts: &mut Vec<SpannerStart>,
    stops: &mut Vec<(SpannerTag, usize, bool)>,
) -> Option<(HookType, f64)> {
    let kind = element.attribute("type").unwrap_or("");
    let placement = placement(element.attribute("placement")).unwrap_or(Placement::Above);
    match kind {
        "start" => {
            let mut info = TextLineInfo::default();
            info.placement = placement;
            info.line_visible = true;
            info.line_style = match element.attribute("line-type") {
                Some("dashed") => TextLineStyle::Dashed,
                Some("dotted") => TextLineStyle::Dotted,
                _ => {
                    if tag == SpannerTag::Dashes {
                        TextLineStyle::Dashed
                    } else {
                        TextLineStyle::Solid
                    }
                }
            };
            info.begin_hook = match element.attribute("line-end") {
                Some("up") | Some("down") => HookType::Hook90,
                Some("both") => HookType::Hook90T,
                Some("arrow") => HookType::Arrow,
                _ => HookType::None,
            };
            if let Some(length) = element
                .attribute("end-length")
                .and_then(|value| value.parse::<f64>().ok())
            {
                info.begin_hook_height = length / 10.0;
            }
            starts.push(SpannerStart {
                tag,
                number,
                kind: SpannerType::TextLine,
                text_line: Some(info),
                pedal: None,
                hairpin: None,
            });
        }
        "stop" => {
            stops.push((tag, number, false));
            let hook = match element.attribute("line-end") {
                Some("up") | Some("down") => HookType::Hook90,
                Some("both") => HookType::Hook90T,
                Some("arrow") => HookType::Arrow,
                _ => HookType::None,
            };
            let height = element
                .attribute("end-length")
                .and_then(|length| length.parse::<f64>().ok())
                .map_or(1.5, |length| length / 10.0);
            return Some((hook, height));
        }
        _ => {}
    }
    None
}

/// Ouvre un spanner et le mémorise par type et numéro.
fn open_spanner(
    score: &mut Score,
    state: &mut DirectionState,
    start: SpannerStart,
    tick: Fraction,
    track: Track,
) {
    let anchor = SpannerAnchor::new(tick, track);
    let mut spanner = Spanner::new(start.kind, anchor, anchor);
    spanner.text_line = start.text_line;
    spanner.pedal = start.pedal;
    spanner.hairpin = start.hairpin;
    let index = score.spanners.len();
    score.spanners.push(spanner);
    state.open.insert((start.tag, start.number), index);
}

/// Ferme un spanner au tick courant.
fn close_spanner(
    score: &mut Score,
    state: &mut DirectionState,
    stop: (SpannerTag, usize, bool),
    tick: Fraction,
    track: Track,
) -> Option<usize> {
    let index = state.open.remove(&(stop.0, stop.1))?;
    let Some(spanner) = score.spanners.get_mut(index) else {
        return None;
    };
    spanner.end = SpannerAnchor::new(tick, track);
    if stop.2 {
        if let Some(hairpin) = spanner.hairpin.as_mut() {
            hairpin.niente = true;
        }
        if stop.0 == SpannerTag::Pedal {
            if let Some(pedal) = spanner.pedal.as_mut() {
                pedal.end_hook = HookType::Hook90;
            }
        }
    }
    Some(index)
}

/// Lit `<sound><instrument-change>` et enregistre le changement.
#[allow(clippy::too_many_arguments)]
fn read_instrument_change(
    score: &mut Score,
    analysis: &Analysis,
    part_index: usize,
    measure_index: usize,
    change: &Element,
    relative_tick: Fraction,
    placement_attribute: Option<&str>,
    state: &mut DirectionState,
) {
    let id = change.attribute("id").unwrap_or_default();
    let Some(data) = analysis.parts[part_index]
        .instruments
        .iter()
        .find(|instrument| instrument.id == id)
        .cloned()
    else {
        return;
    };
    let mut instrument = Instrument {
        id: data.sound.clone(),
        long_name: data.name.clone(),
        short_name: String::new(),
        track_name: data.name.clone(),
        transpose: state.last_transpose,
        midi_program: data.program,
        midi_channel: data.channel,
        volume: data.volume,
        pan: data.pan,
        midi_articulations: Vec::new(),
    };
    if instrument.id.is_empty() {
        instrument.id = data.id.clone();
    }
    let tick = score.measures[measure_index].start_tick + relative_tick;
    score.parts[part_index].set_instrument_at(tick, instrument.clone());
    let placement = placement(placement_attribute).unwrap_or(Placement::Above);
    state.pending_instrument = Some((tick, instrument, placement));
}

/// Reconnaît une reprise et l'ajoute à la mesure (`handleRepeats`).
#[allow(clippy::too_many_arguments)]
fn read_repeats(
    score: &mut Score,
    measure_index: usize,
    _tick: Fraction,
    words: &str,
    symbol_words: &str,
    placement: Option<Placement>,
    track: Track,
    coda: Option<&str>,
    segno: Option<&str>,
    to_coda: Option<&str>,
    fine: bool,
    da_capo: bool,
    dal_segno: Option<&str>,
) -> bool {
    let placement = placement.unwrap_or(Placement::Above);
    let text = if words.is_empty() {
        symbol_words.to_owned()
    } else {
        words.to_owned()
    };
    let mut mark: Option<RepeatMark> = None;
    if let Some(label) = segno {
        mark = Some(RepeatMark::Marker(Marker {
            kind: MarkerType::Segno,
            label: label.to_owned(),
            text: String::new(),
            placement,
            track,
        }));
    } else if let Some(label) = coda {
        mark = Some(RepeatMark::Marker(Marker {
            kind: MarkerType::Coda,
            label: label.to_owned(),
            text: String::new(),
            placement,
            track,
        }));
    } else if fine {
        mark = Some(RepeatMark::Marker(Marker {
            kind: MarkerType::Fine,
            label: "fine".to_owned(),
            text: if text.is_empty() {
                "Fine".to_owned()
            } else {
                text.clone()
            },
            placement,
            track,
        }));
    } else if let Some(label) = to_coda {
        mark = Some(RepeatMark::Marker(Marker {
            kind: MarkerType::ToCoda,
            label: label.to_owned(),
            text: if text.is_empty() {
                "To Coda".to_owned()
            } else {
                text.clone()
            },
            placement,
            track,
        }));
    } else if da_capo {
        let (kind, play_until) = jump_kind(&text, true);
        let mut jump = Jump::new(
            text.clone(),
            "start",
            play_until,
            if kind == mscz2xml::model::JumpType::DcAlCoda {
                "codab"
            } else {
                ""
            },
            placement,
            track,
        );
        jump.kind = kind;
        mark = Some(RepeatMark::Jump(jump));
    } else if let Some(target) = dal_segno {
        let (kind, play_until) = jump_kind(&text, false);
        let mut jump = Jump::new(
            text.clone(),
            target.to_owned(),
            play_until,
            if kind == mscz2xml::model::JumpType::DsAlCoda {
                "codab"
            } else {
                ""
            },
            placement,
            track,
        );
        jump.kind = kind;
        mark = Some(RepeatMark::Jump(jump));
    }
    let Some(mark) = mark else {
        return false;
    };
    score.measures[measure_index].repeat_marks.push(mark);
    true
}

/// Type de saut et cible d'arrêt d'après le texte (`matchRepeat`).
fn jump_kind(text: &str, da_capo: bool) -> (mscz2xml::model::JumpType, String) {
    let lower = text.to_lowercase();
    let al_fine = lower.contains("al fine");
    let al_coda = lower.contains("al coda");
    if da_capo {
        if al_coda {
            (mscz2xml::model::JumpType::DcAlCoda, "codab".to_owned())
        } else if al_fine {
            (mscz2xml::model::JumpType::DcAlFine, "fine".to_owned())
        } else {
            (mscz2xml::model::JumpType::Dc, "end".to_owned())
        }
    } else if al_coda {
        (mscz2xml::model::JumpType::DsAlCoda, "codab".to_owned())
    } else if al_fine {
        (mscz2xml::model::JumpType::DsAlFine, "fine".to_owned())
    } else {
        (mscz2xml::model::JumpType::Ds, "end".to_owned())
    }
}
