// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/repeatlist.cpp` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Déroulement des reprises, voltas et sauts.
//!
//! Portage de `RepeatList` (`engraving/dom/repeatlist.cpp`) : à partir de la
//! liste des mesures du modèle, [`RepeatList::unwind`] produit la suite des
//! [`RepeatSegment`] réellement joués, dans l'ordre d'exécution. Chaque
//! segment connaît son tick source (`tick`), son tick d'exécution (`utick`) et
//! son nombre de passages (`playback_count`).
//!
//! Le déroulement reprend les règles de MuseScore :
//!
//! - reprises simples (`<startRepeat>`/`<endRepeat>`) et compteur `times` ;
//! - voltas, avec un prétraitement des chevauchements ;
//! - sauts D.C./D.S. avec cibles `start`, `end`, marqueurs (`segno`, `coda`,
//!   `fine`, `codab`) et `continueAt` ;
//! - sauts de section : chaque section est déroulée indépendamment.
//!
//! Les ticks sont les ticks internes de MuseScore : [`UNITS_PER_WHOLE`] unités
//! par ronde, soit [`DIVISION`] = 480 unités par noire.
//!
//! Limites connues par rapport au C++ : les cadres (`VBox`/`HBox`) n'existent
//! pas dans le modèle, donc une section commence toujours sur une mesure ; le
//! `playRepeats` d'un saut n'est pas modélisé et vaut faux (valeur par défaut
//! de MuseScore, celle des fichiers du corpus) ; les chevauchements de voltas
//! sont traités de façon simplifiée (les voltas du corpus ne se chevauchent
//! pas) ; la pause d'une barre de section n'est pas conservée.

use std::collections::BTreeSet;

use scoretk_core::model::{Fraction, Jump, RepeatMark, Score, SpannerType};

/// Unités par ronde dans un fichier MIDI de MuseScore
/// (`Constants::DIVISION * 4`).
pub const UNITS_PER_WHOLE: u32 = 1920;

/// Unités par noire (division MIDI de MuseScore, `Constants::DIVISION`).
pub const DIVISION: u32 = 480;

/// Convertit une fraction de ronde en ticks MuseScore.
fn ticks(value: Fraction) -> i64 {
    value.ticks(UNITS_PER_WHOLE)
}

/// Segment de reprise : mesures jouées d'un seul tenant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatSegment {
    /// Tick source du début du segment.
    pub tick: i64,
    /// Tick d'exécution du début du segment.
    pub utick: i64,
    /// Numéro de passage (le premier vaut 1).
    pub playback_count: i32,
    /// Indices des mesures jouées, dans l'ordre (`Score::measures`).
    pub measures: Vec<usize>,
}

impl RepeatSegment {
    fn new(tick: i64, playback_count: i32) -> Self {
        Self {
            tick,
            utick: 0,
            playback_count,
            measures: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.measures.is_empty()
    }

    fn add_measure(&mut self, measure: usize, starts: &[i64]) {
        if self.measures.is_empty() {
            self.tick = starts[measure];
        }
        if self.measures.last() != Some(&measure) {
            self.measures.push(measure);
        }
    }

    /// Ajoute ou retire des mesures pour terminer exactement sur `measure`
    /// (`RepeatSegment::addMeasures`).
    fn add_measures(&mut self, measure: usize, starts: &[i64]) {
        if let Some(last) = self.measures.last().copied() {
            for next in (last + 1)..measure {
                self.measures.push(next);
            }
        }
        while self.measures.last().is_some_and(|last| *last >= measure) {
            self.measures.pop();
        }
        self.add_measure(measure, starts);
    }

    fn pop_measure(&mut self) {
        self.measures.pop();
    }

    /// Premier indice de mesure du segment.
    pub fn first_measure(&self) -> Option<usize> {
        self.measures.first().copied()
    }

    /// Dernier indice de mesure du segment.
    pub fn last_measure(&self) -> Option<usize> {
        self.measures.last().copied()
    }
}

/// Volta prétraitée.
#[derive(Clone, Debug, PartialEq, Eq)]
struct VoltaSpan {
    /// Première mesure couverte (incluse).
    start: usize,
    /// Dernière mesure couverte (incluse).
    end: usize,
    /// Numéros d'ending.
    endings: Vec<u8>,
    /// Vrai si la volta est fermée (`endHookType` non nul).
    closed: bool,
}

impl VoltaSpan {
    fn has_ending(&self, playback_count: i32) -> bool {
        self.endings
            .iter()
            .any(|ending| i32::from(*ending) == playback_count)
    }

    fn first_ending(&self) -> i32 {
        self.endings.first().map_or(0, |ending| i32::from(*ending))
    }

    fn last_ending(&self) -> i32 {
        self.endings.last().map_or(0, |ending| i32::from(*ending))
    }
}

/// Type d'élément qui influe sur l'ordre de lecture
/// (`RepeatListElementType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementKind {
    SectionBreak,
    VoltaStart,
    VoltaEnd,
    RepeatStart,
    RepeatEnd,
    Jump,
    Marker,
}

/// Élément de la liste de lecture (`RepeatListElement`).
#[derive(Clone, Debug)]
struct Element {
    kind: ElementKind,
    /// Mesure qui contient l'élément.
    measure: usize,
    /// Indice dans la liste des voltas prétraitées.
    volta: Option<usize>,
    /// Libellé du marqueur, pour `find_marker`.
    marker_label: Option<String>,
    /// Vrai pour un marqueur de fin de mesure (`isRightMarker`), pour
    /// l'insertion des marqueurs (`repeatlist.cpp:549-558`).
    marker_is_right: bool,
    /// Copie du saut (le modèle ne prête pas ses mesures).
    jump: Option<Jump>,
    /// Indice du saut, pour `m_jumps_taken` et `active_jump`.
    jump_index: Option<usize>,
    /// Compteur `times` de la mesure (`Measure::repeatCount()`), pour les
    /// `REPEAT_END`.
    measure_repeat_count: i32,
    /// Nombre de lectures déjà effectuées (`repeatCount`).
    repeat_count: i32,
}

impl Element {
    fn new(kind: ElementKind, measure: usize) -> Self {
        Self {
            kind,
            measure,
            volta: None,
            marker_label: None,
            marker_is_right: false,
            jump: None,
            jump_index: None,
            measure_repeat_count: 0,
            repeat_count: if kind == ElementKind::RepeatStart {
                1
            } else {
                0
            },
        }
    }
}

/// Liste déroulée des segments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepeatList {
    /// Segments dans l'ordre d'exécution.
    pub segments: Vec<RepeatSegment>,
    /// Tick de début de chaque mesure (ticks MuseScore).
    starts: Vec<i64>,
    /// Tick de fin de chaque mesure (ticks MuseScore).
    ends: Vec<i64>,
}

impl RepeatList {
    /// Déroule la partition sans développer les reprises (`flatten`) : un seul
    /// segment qui couvre toutes les mesures.
    pub fn flatten(score: &Score) -> Self {
        let (starts, ends) = measure_ticks(score);
        let mut list = Self {
            segments: Vec::new(),
            starts,
            ends,
        };
        if score.measures.is_empty() {
            return list;
        }
        let mut segment = RepeatSegment::new(list.starts[0], 1);
        for measure in 0..score.measures.len() {
            segment.add_measure(measure, &list.starts);
        }
        list.segments.push(segment);
        list.update_uticks();
        list
    }

    /// Déroule la partition en développant reprises, voltas et sauts
    /// (`RepeatList::unwind`).
    pub fn unwind(score: &Score) -> Self {
        let (starts, ends) = measure_ticks(score);
        let mut list = Self {
            segments: Vec::new(),
            starts,
            ends,
        };
        if score.measures.is_empty() {
            return list;
        }

        let voltas = collect_voltas(score, &list.starts);
        let mut sections = collect_sections(score, &voltas);
        list.unwind_sections(&mut sections, &voltas);
        list.update_uticks();
        list
    }

    fn update_uticks(&mut self) {
        let lens: Vec<i64> = self
            .segments
            .iter()
            .map(|segment| self.segment_len(segment))
            .collect();
        let mut utick = 0;
        for (segment, len) in self.segments.iter_mut().zip(lens) {
            segment.utick = utick;
            utick += len;
        }
    }

    /// Longueur d'un segment en ticks.
    pub fn segment_len(&self, segment: &RepeatSegment) -> i64 {
        let Some(last) = segment.last_measure() else {
            return 0;
        };
        self.ends[last] - segment.tick
    }

    /// Tick de fin (exclu) d'un segment.
    pub fn segment_end(&self, segment: &RepeatSegment) -> i64 {
        segment.tick + self.segment_len(segment)
    }

    /// Tick de début d'une mesure (ticks MuseScore).
    pub fn measure_start(&self, measure: usize) -> i64 {
        self.starts[measure]
    }

    /// Tick de fin d'une mesure (ticks MuseScore).
    pub fn measure_end(&self, measure: usize) -> i64 {
        self.ends[measure]
    }

    /// Nombre total de ticks joués (`RepeatList::ticks`).
    pub fn ticks(&self) -> i64 {
        self.segments
            .last()
            .map_or(0, |segment| segment.utick + self.segment_len(segment))
    }

    /// Tick source correspondant à un tick d'exécution
    /// (`RepeatList::utick2tick`).
    pub fn utick2tick(&self, utick: i64) -> i64 {
        if self.segments.is_empty() {
            return utick;
        }
        for (index, segment) in self.segments.iter().enumerate() {
            let end = self.segment_end(segment);
            if utick >= segment.utick && (utick < end || index + 1 == self.segments.len()) {
                return utick - (segment.utick - segment.tick);
            }
        }
        0
    }

    /// Tick d'exécution correspondant à un tick source
    /// (`RepeatList::tick2utick`). Au-delà du dernier segment, la translation
    /// du dernier s'applique, comme dans le C++.
    pub fn tick2utick(&self, tick: i64) -> i64 {
        let Some(last) = self.segments.last() else {
            return 0;
        };
        for segment in &self.segments {
            if tick >= segment.tick && tick < self.segment_end(segment) {
                return segment.utick + (tick - segment.tick);
            }
        }
        last.utick + (tick - last.tick)
    }

    fn push_segment(&mut self, segment: &mut Option<RepeatSegment>) {
        if let Some(current) = segment.take() {
            if !current.is_empty() {
                self.segments.push(current);
            }
        }
    }

    fn unwind_sections(&mut self, sections: &mut [Vec<Element>], voltas: &[VoltaSpan]) {
        // `m_jumpsTaken` : un saut n'est pris qu'une fois par passage.
        let mut jumps_taken: BTreeSet<(usize, usize, i32)> = BTreeSet::new();

        let mut section = 0usize;
        while section < sections.len() {
            let mut playback_count = 1;
            let mut it = 0usize;
            let mut start_repeat = (section, 0usize);
            let mut active_volta: Option<usize> = None;
            let mut play_until: Option<(usize, usize)> = None;
            let mut continue_at: Option<(usize, usize)> = None;
            let mut active_jump: Option<usize> = None;
            let mut force_final_repeat = false;

            let first_measure = sections[section][0].measure;
            let mut segment = Some(RepeatSegment::new(
                self.starts[first_measure],
                playback_count,
            ));
            if let Some(current) = segment.as_mut() {
                current.add_measure(first_measure, &self.starts);
            }
            it += 1;

            while it < sections[section].len() {
                let info = sections[section][it].clone();
                if let Some(current) = segment.as_mut() {
                    current.add_measures(info.measure, &self.starts);
                }

                let mut rewind = false;
                match info.kind {
                    ElementKind::SectionBreak => {
                        self.push_segment(&mut segment);
                    }
                    ElementKind::VoltaStart => {
                        let volta = info.volta.expect("un VOLTA_START porte une volta");
                        active_volta = Some(volta);
                        if !voltas[volta].has_ending(playback_count) {
                            // Ending non joué : retirer la mesure et sauter
                            // jusqu'à la fin de la volta.
                            if let Some(current) = segment.as_mut() {
                                current.pop_measure();
                            }
                            self.push_segment(&mut segment);
                            while sections[section][it].kind != ElementKind::VoltaEnd {
                                it += 1;
                            }
                            active_volta = None;
                            let end_measure = sections[section][it].measure;
                            if end_measure + 1 < self.starts.len() {
                                let mut next = RepeatSegment::new(
                                    self.starts[end_measure + 1],
                                    playback_count,
                                );
                                next.add_measure(end_measure + 1, &self.starts);
                                segment = Some(next);
                            } else {
                                segment = None;
                            }
                        }
                    }
                    ElementKind::VoltaEnd => {
                        active_volta = None;
                    }
                    ElementKind::RepeatStart => {
                        if segment.is_none() {
                            let mut next =
                                RepeatSegment::new(self.starts[info.measure], playback_count);
                            next.add_measure(info.measure, &self.starts);
                            segment = Some(next);
                        } else {
                            let desired = if force_final_repeat {
                                info.repeat_count
                            } else {
                                1
                            };
                            let current_count = segment
                                .as_ref()
                                .map_or(playback_count, |current| current.playback_count);
                            if current_count != desired {
                                if let Some(current) = segment.as_mut() {
                                    current.pop_measure();
                                }
                                self.push_segment(&mut segment);
                                playback_count = desired;
                                let mut next =
                                    RepeatSegment::new(self.starts[info.measure], playback_count);
                                next.add_measure(info.measure, &self.starts);
                                segment = Some(next);
                            }
                        }
                        start_repeat = (section, it);
                    }
                    ElementKind::RepeatEnd => {
                        sections[section][it].repeat_count += 1;
                        let new_count = sections[section][it].repeat_count;
                        let start_count = sections[start_repeat.0][start_repeat.1].repeat_count;
                        if playback_count < start_count && new_count < info.measure_repeat_count {
                            self.push_segment(&mut segment);
                            segment = None;
                            // Remonter jusqu'au début de la reprise, en
                            // rétablissant la volta active.
                            loop {
                                it -= 1;
                                match sections[section][it].kind {
                                    ElementKind::VoltaStart => active_volta = None,
                                    ElementKind::VoltaEnd => {
                                        active_volta = sections[section][it].volta;
                                    }
                                    _ => {}
                                }
                                if (section, it) == start_repeat {
                                    break;
                                }
                            }
                            playback_count += 1;
                            rewind = true;
                        }
                    }
                    ElementKind::Jump => {
                        let jump = info.jump.as_ref().expect("un saut porte ses cibles");
                        let jump_index = info.jump_index.expect("un saut a un indice");
                        // Même saut atteint de nouveau après l'avoir pris.
                        if active_jump == Some(jump_index) {
                            force_final_repeat = false;
                        }
                        let start_count = sections[start_repeat.0][start_repeat.1].repeat_count;
                        let reached_final = playback_count >= start_count
                            || active_volta
                                .is_some_and(|volta| playback_count == voltas[volta].last_ending());
                        if reached_final && jumps_taken.insert((section, it, playback_count)) {
                            let jump_to = find_marker(sections, jump.jump_to.as_str(), section, it);
                            let until =
                                find_marker(sections, jump.play_until.as_str(), section, it);
                            let cont =
                                find_marker(sections, jump.continue_at.as_str(), section, it);
                            play_until = until;
                            continue_at = cont;

                            if let Some(target) = jump_to {
                                self.push_segment(&mut segment);
                                active_jump = Some(jump_index);
                                let (count, volta, start) =
                                    perform_jump(sections, voltas, target, false, start_repeat);
                                playback_count = count;
                                active_volta = volta;
                                start_repeat = start;
                                section = target.0;
                                it = target.1;
                                force_final_repeat = true;

                                if play_until.is_some() {
                                    let mut index = it + 1;
                                    while index < sections[section].len() {
                                        let element = &mut sections[section][index];
                                        if element.kind == ElementKind::RepeatEnd
                                            && element.repeat_count != 0
                                        {
                                            element.repeat_count = 0;
                                            if force_final_repeat {
                                                element.repeat_count = element.measure_repeat_count;
                                            }
                                        }
                                        index += 1;
                                    }
                                }

                                if let Some(volta) = active_volta {
                                    if playback_count
                                        < sections[start_repeat.0][start_repeat.1].repeat_count
                                    {
                                        reevaluate_volta_repeat(
                                            &mut sections[section],
                                            voltas,
                                            start_repeat.1,
                                            volta,
                                            playback_count,
                                        );
                                    }
                                }

                                let mut next = RepeatSegment::new(
                                    self.starts[sections[section][it].measure],
                                    playback_count,
                                );
                                next.add_measure(sections[section][it].measure, &self.starts);
                                segment = Some(next);
                            }
                        }
                    }
                    ElementKind::Marker => {
                        let start_count = sections[start_repeat.0][start_repeat.1].repeat_count;
                        let reached_final = playback_count >= start_count
                            || active_volta
                                .is_some_and(|volta| playback_count == voltas[volta].last_ending());
                        if play_until == Some((section, it)) && reached_final {
                            self.push_segment(&mut segment);
                            play_until = None;
                            force_final_repeat = false;

                            if let Some(target) = continue_at {
                                let (count, volta, start) =
                                    perform_jump(sections, voltas, target, true, start_repeat);
                                playback_count = count;
                                active_volta = volta;
                                start_repeat = start;
                                section = target.0;
                                it = target.1;
                                if sections[start_repeat.0][start_repeat.1].measure
                                    != sections[section][it].measure
                                {
                                    start_repeat = (section, it);
                                }
                                let mut next = RepeatSegment::new(
                                    self.starts[sections[section][it].measure],
                                    playback_count,
                                );
                                next.add_measure(sections[section][it].measure, &self.starts);
                                segment = Some(next);
                                continue_at = None;
                                it += 1;
                                continue;
                            }
                            it = sections[section].len();
                            continue;
                        }
                    }
                }

                if !rewind {
                    it += 1;
                }
            }

            section += 1;
        }
    }
}

/// Tick de début et de fin de chaque mesure.
fn measure_ticks(score: &Score) -> (Vec<i64>, Vec<i64>) {
    let starts: Vec<i64> = score
        .measures
        .iter()
        .map(|measure| ticks(measure.start_tick))
        .collect();
    let ends: Vec<i64> = score
        .measures
        .iter()
        .map(|measure| ticks(measure.start_tick + measure.len))
        .collect();
    (starts, ends)
}

/// Mesure contenant un tick (la dernière dont le début est antérieur ou égal).
fn measure_at(starts: &[i64], tick: i64) -> Option<usize> {
    starts
        .iter()
        .rposition(|start| *start <= tick)
        .filter(|index| *index < starts.len())
}

/// Voltas du modèle, dédoublonnées et converties en indices de mesures.
fn collect_voltas(score: &Score, starts: &[i64]) -> Vec<VoltaSpan> {
    let mut voltas = Vec::new();
    for spanner in &score.spanners {
        if !matches!(spanner.kind, SpannerType::Volta) {
            continue;
        }
        let start_tick = ticks(spanner.start.tick);
        let end_tick = ticks(spanner.end.tick);
        let Some(start) = measure_at(starts, start_tick) else {
            continue;
        };
        // La fin d'un spanner est le début de la mesure suivante : la dernière
        // mesure couverte est celle qui contient `end_tick - 1`.
        let Some(end) = measure_at(starts, (end_tick - 1).max(start_tick)) else {
            continue;
        };
        let info = spanner.volta.clone().unwrap_or_default();
        let span = VoltaSpan {
            start,
            end,
            endings: info.endings.clone(),
            closed: info.closed,
        };
        if !voltas.contains(&span) {
            voltas.push(span);
        }
    }
    voltas.sort_by_key(|volta| (volta.start, volta.end));
    voltas
}

/// Ferme la volta active en poussant un `VOLTA_END` sur la mesure `measure`,
/// puis la retire (`repeatlist.cpp:490`, `:509`, `:535`, `:584`, `:597`,
/// `:607`).
fn close_volta(elements: &mut Vec<Element>, volta_active: &mut Option<usize>, measure: usize) {
    if let Some(volta) = volta_active.take() {
        let mut element = Element::new(ElementKind::VoltaEnd, measure);
        element.volta = Some(volta);
        elements.push(element);
    }
}

/// Construit la liste des éléments par section
/// (`RepeatList::collectRepeatListElements`).
fn collect_sections(score: &Score, voltas: &[VoltaSpan]) -> Vec<Vec<Element>> {
    // Découpage en sections : une section commence à la mesure qui suit un
    // saut de section, ou à la première mesure.
    let mut section_bounds = vec![0usize];
    for (index, measure) in score.measures.iter().enumerate().skip(1) {
        if measure.section_break_before {
            section_bounds.push(index);
        }
    }

    let mut jump_index = 0usize;
    let mut sections = Vec::new();
    for (section_number, &start) in section_bounds.iter().enumerate() {
        let end = section_bounds
            .get(section_number + 1)
            .map_or(score.measures.len() - 1, |next| next - 1);
        let mut elements = vec![Element::new(ElementKind::RepeatStart, start)];
        let mut volta_active: Option<usize> = None;

        for measure_index in start..=end {
            let measure = &score.measures[measure_index];

            // Début de volta : fermer celle qui serait encore ouverte.
            let starting: Vec<usize> = voltas
                .iter()
                .enumerate()
                .filter(|(_, volta)| volta.start == measure_index)
                .map(|(index, _)| index)
                .collect();
            for volta_index in starting {
                if volta_active.is_some_and(|previous| previous != volta_index) {
                    close_volta(
                        &mut elements,
                        &mut volta_active,
                        measure_index.saturating_sub(1),
                    );
                }
                let mut element = Element::new(ElementKind::VoltaStart, measure_index);
                element.volta = Some(volta_index);
                elements.push(element);
                volta_active = Some(volta_index);
            }

            // Début de reprise.
            if measure.start_bar_line.is_repeat_start() {
                if volta_active.is_some_and(|volta| voltas[volta].start != measure_index) {
                    close_volta(
                        &mut elements,
                        &mut volta_active,
                        measure_index.saturating_sub(1),
                    );
                }
                elements.push(Element::new(ElementKind::RepeatStart, measure_index));
            }

            // Sauts et marqueurs, dans l'ordre de `el()`
            // (`repeatlist.cpp:517-574`). Le C++ pousse le saut immédiatement,
            // puis insère le `VOLTA_END` qui le précède éventuellement juste
            // après lui : les éléments suivants de `el()` (autres sauts,
            // marqueurs) viennent après. Les marqueurs sont insérés avant les
            // sauts de la même mesure (`repeatlist.cpp:545-573`).
            for mark in &measure.repeat_marks {
                match mark {
                    RepeatMark::Jump(jump) => {
                        let mut element = Element::new(ElementKind::Jump, measure_index);
                        element.jump = Some(jump.clone());
                        element.jump_index = Some(jump_index);
                        jump_index += 1;
                        elements.push(element);

                        // Un saut ferme la volta encore ouverte qui s'est
                        // terminée avant lui, ou qui est ouverte et se
                        // termine sur cette mesure (`repeatlist.cpp:525-543`)
                        // — sauf si la mesure porte aussi l'`endRepeat`
                        // (cf. #327681), auquel cas c'est la fin de reprise
                        // qui la ferme.
                        if let Some(volta) = volta_active {
                            let span = &voltas[volta];
                            let ends_here = span.end < measure_index
                                || (span.end == measure_index && !span.closed);
                            if ends_here && !measure.end_bar_line.is_repeat_end() {
                                close_volta(&mut elements, &mut volta_active, measure_index);
                            }
                        }
                    }
                    RepeatMark::Marker(marker) => {
                        let right = marker.kind.is_stop_marker();
                        let mut element = Element::new(ElementKind::Marker, measure_index);
                        element.marker_label = Some(marker.label.clone());
                        element.marker_is_right = right;
                        // Reculer devant les sauts et les marqueurs de droite
                        // de la même mesure (`repeatlist.cpp:547-572`).
                        let mut insertion = elements.len().saturating_sub(1);
                        while !elements.is_empty() && elements[insertion].measure == measure_index {
                            let (kind, stored_right) = {
                                let stored = &elements[insertion];
                                (stored.kind, stored.marker_is_right)
                            };
                            let should_go_before =
                                !right && kind == ElementKind::Marker && stored_right;
                            if should_go_before || kind == ElementKind::Jump {
                                if insertion == 0 {
                                    break;
                                }
                                insertion -= 1;
                            } else {
                                break;
                            }
                        }
                        elements.insert((insertion + 1).min(elements.len()), element);
                    }
                }
            }

            // Fin de reprise.
            if measure.end_bar_line.is_repeat_end() {
                let mut element = Element::new(ElementKind::RepeatEnd, measure_index);
                element.measure_repeat_count = i32::from(measure.repeat_count.max(2));
                elements.push(element);
                if let Some(start_element) = elements
                    .iter_mut()
                    .rev()
                    .find(|element| element.kind == ElementKind::RepeatStart)
                {
                    start_element.repeat_count += i32::from(measure.repeat_count.max(2)) - 1;
                }
                // Le C++ ferme ici **toute** volta encore ouverte, sans
                // condition sur sa mesure de fin (`repeatlist.cpp:581-588`) :
                // une volta qui déborde l'`endRepeat` se termine sur cette
                // mesure.
                close_volta(&mut elements, &mut volta_active, measure_index);
            }

            // Fin de volta : fermée à sa dernière mesure, ou ouverte et suivie
            // d'une autre volta, d'une reprise ou d'une section.
            if let Some(volta) = volta_active {
                let span = &voltas[volta];
                let next_volta_starts = voltas
                    .iter()
                    .any(|other| other.start == measure_index + 1 && other.start != span.start);
                let ends_here = span.end == measure_index
                    && (span.closed
                        || next_volta_starts
                        || measure_index == end
                        || score
                            .measures
                            .get(measure_index + 1)
                            .is_some_and(|next| next.section_break_before));
                if ends_here {
                    close_volta(&mut elements, &mut volta_active, measure_index);
                }
            }
        }

        // Fin de section.
        close_volta(&mut elements, &mut volta_active, end);
        elements.push(Element::new(ElementKind::SectionBreak, end));
        sections.push(elements);
    }
    sections
}

/// Recherche d'un marqueur (`RepeatList::findMarker`). Les libellés `start` et
/// `end` désignent les extrémités de la section courante.
fn find_marker(
    sections: &[Vec<Element>],
    label: &str,
    reference_section: usize,
    reference_element: usize,
) -> Option<(usize, usize)> {
    if label.is_empty() {
        return None;
    }
    let section = &sections[reference_section];
    if label == "start" {
        return Some((reference_section, 0));
    }
    if label == "end" {
        return Some((reference_section, section.len() - 1));
    }

    // Recherche en arrière, puis en avant dans la section courante.
    let mut index = reference_element;
    while index > 0 {
        index -= 1;
        if section[index].marker_label.as_deref() == Some(label) {
            return Some((reference_section, index));
        }
    }
    for (index, element) in section.iter().enumerate().skip(reference_element + 1) {
        if element.marker_label.as_deref() == Some(label) {
            return Some((reference_section, index));
        }
    }

    // Sections précédentes, en arrière.
    let mut current = reference_section;
    while current > 0 {
        current -= 1;
        for (index, element) in sections[current].iter().enumerate().rev() {
            if element.marker_label.as_deref() == Some(label) {
                return Some((current, index));
            }
        }
    }

    // Sections suivantes, en avant.
    for (current, elements) in sections.iter().enumerate().skip(reference_section + 1) {
        for (index, element) in elements.iter().enumerate() {
            if element.marker_label.as_deref() == Some(label) {
                return Some((current, index));
            }
        }
    }
    None
}

/// Exécute un saut (`RepeatList::performJump`) et renvoie le nouveau compteur
/// de passage, la volta active et le point de reprise de référence.
fn perform_jump(
    sections: &[Vec<Element>],
    voltas: &[VoltaSpan],
    target: (usize, usize),
    with_repeats: bool,
    _start_repeat: (usize, usize),
) -> (i32, Option<usize>, (usize, usize)) {
    let mut active_volta = None;
    let mut start = (target.0, 0usize);
    for (index, element) in sections[target.0][..target.1].iter().enumerate() {
        match element.kind {
            ElementKind::VoltaStart => active_volta = element.volta,
            ElementKind::VoltaEnd => active_volta = None,
            ElementKind::RepeatStart => start = (target.0, index),
            _ => {}
        }
    }

    let playback_count = if with_repeats {
        active_volta
            .map(|volta| voltas[volta].first_ending().max(1))
            .unwrap_or(1)
    } else {
        active_volta.map_or_else(
            || sections[start.0][start.1].repeat_count,
            |volta| {
                let last = voltas[volta].last_ending();
                if last == 0 {
                    sections[start.0][start.1].repeat_count
                } else {
                    last
                }
            },
        )
    };
    (playback_count, active_volta, start)
}

/// Réévalue les fins de reprise après être entré dans une volta par un saut
/// (`test::repeat52`/`repeat53` en amont).
fn reevaluate_volta_repeat(
    elements: &mut [Element],
    voltas: &[VoltaSpan],
    start_repeat: usize,
    active_volta: usize,
    playback_count: i32,
) {
    let start_count = elements[start_repeat].repeat_count;
    let mut index = start_repeat + 1;
    let mut processed = 1;
    let mut volta_reference = Some(active_volta);
    while index < elements.len()
        && elements[index].kind != ElementKind::RepeatStart
        && processed < start_count
    {
        match elements[index].kind {
            ElementKind::VoltaStart => {
                let volta = elements[index].volta.unwrap_or(active_volta);
                volta_reference = if voltas[volta].last_ending() < playback_count {
                    None
                } else {
                    Some(volta)
                };
            }
            ElementKind::RepeatEnd => {
                if let Some(volta) = volta_reference {
                    let remaining = voltas[volta]
                        .endings
                        .iter()
                        .filter(|ending| i32::from(**ending) >= playback_count)
                        .count() as i32;
                    let measure_count = elements[index].measure_repeat_count;
                    elements[index].repeat_count = measure_count - remaining - 1;
                    processed += measure_count - 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        BarLineType, Jump, Marker, MarkerType, Measure, Placement, RepeatMark, Spanner,
        SpannerAnchor, SpannerType, Track, VoltaInfo,
    };

    fn measure(number: usize, tick: Fraction, len: Fraction) -> Measure {
        Measure::new(number, tick, len)
    }

    /// Une mesure unique portant une volta ouverte et les marques données,
    /// dans l'ordre où `Measure::el()` les expose.
    fn sections_avec_volta(marks: Vec<RepeatMark>) -> Vec<Vec<Element>> {
        let mut score = Score::new();
        let whole = Fraction::ONE;
        let mut value = measure(0, Fraction::from_integer(0), whole);
        value.repeat_marks = marks;
        score.add_measure(value);
        let mut volta = Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(Fraction::from_integer(0), Track::from_raw(0)),
            SpannerAnchor::new(whole, Track::from_raw(0)),
        );
        volta.volta = Some(VoltaInfo {
            endings: vec![1],
            closed: false,
        });
        score.add_spanner(volta);
        let (starts, _) = measure_ticks(&score);
        let voltas = collect_voltas(&score, &starts);
        collect_sections(&score, &voltas)
    }

    fn jump() -> Jump {
        Jump::new(
            "D.C.",
            "start",
            "",
            "",
            Placement::Above,
            Track::from_raw(0),
        )
    }

    fn marker() -> Marker {
        Marker {
            kind: MarkerType::Segno,
            label: "segno".into(),
            text: "Segno".into(),
            placement: Placement::Above,
            track: Track::from_raw(0),
        }
    }

    fn kinds(section: &[Element]) -> Vec<ElementKind> {
        section.iter().map(|element| element.kind).collect()
    }

    /// Le C++ pousse le `JUMP`, puis le `VOLTA_END` juste après lui, dans la
    /// boucle sur `el()` ; le marqueur qui suit vient après
    /// (`repeatlist.cpp:523-543`).
    #[test]
    fn volta_end_apres_le_saut_avant_le_marqueur() {
        let sections =
            sections_avec_volta(vec![RepeatMark::Jump(jump()), RepeatMark::Marker(marker())]);
        assert_eq!(
            kinds(&sections[0]),
            vec![
                ElementKind::RepeatStart,
                ElementKind::VoltaStart,
                ElementKind::Jump,
                ElementKind::VoltaEnd,
                ElementKind::Marker,
                ElementKind::SectionBreak,
            ]
        );
    }

    /// Avec deux sauts, c'est toujours juste après le premier que le
    /// `VOLTA_END` est inséré ; le second saut vient ensuite.
    #[test]
    fn volta_end_apres_le_premier_de_deux_sauts() {
        let sections =
            sections_avec_volta(vec![RepeatMark::Jump(jump()), RepeatMark::Jump(jump())]);
        assert_eq!(
            kinds(&sections[0]),
            vec![
                ElementKind::RepeatStart,
                ElementKind::VoltaStart,
                ElementKind::Jump,
                ElementKind::VoltaEnd,
                ElementKind::Jump,
                ElementKind::SectionBreak,
            ]
        );
    }

    #[test]
    fn sans_reprise_un_seul_segment() {
        let mut score = Score::new();
        let whole = Fraction::ONE;
        for number in 0..3 {
            score.add_measure(measure(
                number,
                whole * Fraction::from_integer(number as i64),
                whole,
            ));
        }
        let list = RepeatList::unwind(&score);
        assert_eq!(list.segments.len(), 1);
        assert_eq!(list.segments[0].measures, vec![0, 1, 2]);
        assert_eq!(list.ticks(), 3 * 1920);
    }

    /// Volta ouverte qui déborde la mesure d'`endRepeat` : le C++ la ferme à
    /// la mesure du `RepeatEnd` (`repeatlist.cpp:581-588`), donc la mesure qui
    /// suit l'`endRepeat` est bien jouée au second passage.
    #[test]
    fn volta_debordant_le_repeat_end() {
        let mut score = Score::new();
        let whole = Fraction::ONE;
        for number in 0..6 {
            let mut value = measure(number, whole * Fraction::from_integer(number as i64), whole);
            if number == 0 {
                value.start_bar_line = BarLineType::StartRepeat;
            }
            if number == 2 {
                value.end_bar_line = BarLineType::EndRepeat;
                value.repeat_count = 2;
            }
            score.add_measure(value);
        }
        // Volta 1 sur les mesures 1 à 3, ouverte : sa dernière mesure dépasse
        // l'`endRepeat` (mesure 2).
        let mut volta = Spanner::new(
            SpannerType::Volta,
            SpannerAnchor::new(Fraction::from_integer(1), Track::from_raw(0)),
            SpannerAnchor::new(Fraction::from_integer(4), Track::from_raw(0)),
        );
        volta.volta = Some(VoltaInfo {
            endings: vec![1],
            closed: false,
        });
        score.add_spanner(volta);

        let list = RepeatList::unwind(&score);
        assert_eq!(list.segments.len(), 3);
        // Premier passage : reprise de 0 à 2, volta 1 jouée.
        assert_eq!(list.segments[0].measures, vec![0, 1, 2]);
        // Second passage : la volta 1 est sautée, la mesure 3 est rejouée.
        assert_eq!(list.segments[1].measures, vec![0]);
        assert_eq!(list.segments[2].measures, vec![3, 4, 5]);
        assert_eq!(list.ticks(), 7 * 1920);
    }

    #[test]
    fn reprise_deux_fois() {
        let mut score = Score::new();
        let whole = Fraction::ONE;
        for number in 0..3 {
            let mut value = measure(number, whole * Fraction::from_integer(number as i64), whole);
            if number == 0 {
                value.start_bar_line = BarLineType::StartRepeat;
            }
            if number == 1 {
                value.end_bar_line = BarLineType::EndRepeat;
                value.repeat_count = 2;
            }
            score.add_measure(value);
        }
        let list = RepeatList::unwind(&score);
        assert_eq!(list.segments.len(), 2);
        assert_eq!(list.segments[0].measures, vec![0, 1]);
        assert_eq!(list.segments[1].measures, vec![0, 1, 2]);
        assert_eq!(list.segments[1].utick, 2 * 1920);
    }
}
