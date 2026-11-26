// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rendering/score/beamlayout.cpp`,
// `engraving/dom/groups.cpp` et
// `importexport/musicxml/internal/export/exportmusicxml.cpp` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Ligatures : groupement automatique et écriture des `<beam>`.
//!
//! MuseScore ne stocke pas les ligatures dans le `.mscx` : elles sont créées
//! à la mise en page par `BeamLayout::createBeams`
//! (`rendering/score/beamlayout.cpp`), à partir du mode de ligature de chaque
//! accord (`ChordRest::beamMode()`), de la mesure et des groupes de la
//! signature (`Groups::baseBeamMode` / `actualBeamMode`,
//! `engraving/dom/groups.cpp`). L'écriture MusicXML
//! (`writeBeam` dans `exportmusicxml.cpp`) déduit ensuite, niveau par niveau,
//! `begin`/`continue`/`end`/`forward hook`/`backward hook` du nombre de
//! crochets de l'élément et de ses voisins.
//!
//! Ce module reconstitue les deux étapes : il ne modifie pas le modèle, il
//! produit pour chaque `(mesure, piste)` la liste des `<beam>` à écrire et la
//! direction de hampe imposée par la ligature.

use std::collections::{BTreeMap, BTreeSet};

use scoretk_core::model::{
    BeamMode, Chord, ChordRest, Duration, DurationType, Fraction, Measure, Score, SegmentItem,
    SegmentType, TimeSig, Track, VOICES,
};

use super::stems;

/// Informations de ligature d'un accord ou silence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BeamInfo {
    /// Vrai si l'élément fait partie d'une ligature d'au moins deux éléments.
    pub beamed: bool,
    /// Direction de hampe imposée par la ligature (`Beam::up()`).
    pub up: bool,
    /// Texte de chaque niveau, du niveau 1 au plus haut (`begin`, `continue`…).
    pub texts: Vec<String>,
    /// Index de l'objet `Beam` auquel appartient l'élément.
    pub group: usize,
}

/// Un objet `Beam` reconstitué (`Beam::elements()`).
#[derive(Clone, Debug)]
pub(crate) struct BeamGroup {
    /// Premier élément de la ligature (`beam->elements().front()`).
    pub start: CrRef,
    /// Dernier élément de la ligature (`beam->elements().back()`).
    pub end: CrRef,
}

/// Position d'un élément rythmique dans la partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CrRef {
    pub measure: usize,
    pub track: Track,
    pub tick: Fraction,
}

/// Ligatures calculées pour toute la partition.
pub(crate) struct Beams {
    /// `entries[mesure][piste][tick]`.
    entries: Vec<Vec<BTreeMap<Fraction, BeamInfo>>>,
    /// Objets `Beam` reconstitués, dans l'ordre de création.
    groups: Vec<BeamGroup>,
}

impl Beams {
    /// Reconstitue les ligatures de la partition.
    pub(crate) fn new(score: &Score, concert_pitch: bool) -> Self {
        let tracks = score.staves.len() * VOICES;
        let mut beams = Self {
            entries: vec![vec![BTreeMap::new(); tracks]; score.measures.len()],
            groups: Vec::new(),
        };
        for track in 0..tracks {
            beams.compute_track(score, Track::from_raw(track as u32), concert_pitch);
        }
        beams
    }

    /// Informations de ligature d'un élément, s'il en a.
    pub(crate) fn info(&self, measure: usize, track: Track, tick: Fraction) -> Option<&BeamInfo> {
        self.entries
            .get(measure)
            .and_then(|row| row.get(track.index()))
            .and_then(|map| map.get(&tick))
            .filter(|info| info.beamed)
    }

    /// Direction de hampe imposée par une ligature.
    pub(crate) fn up(&self, measure: usize, track: Track, tick: Fraction) -> Option<bool> {
        self.info(measure, track, tick).map(|info| info.up)
    }

    /// Textes des niveaux de ligature d'un élément.
    pub(crate) fn texts(&self, measure: usize, track: Track, tick: Fraction) -> &[String] {
        self.info(measure, track, tick)
            .map_or(&[], |info| &info.texts)
    }

    /// Objet `Beam` auquel appartient un élément, pour
    /// `Tuplet::calcHasBracket` (`tuplet.cpp:218-224`).
    pub(crate) fn group_of(
        &self,
        measure: usize,
        track: Track,
        tick: Fraction,
    ) -> Option<&BeamGroup> {
        let index = self
            .entries
            .get(measure)
            .and_then(|row| row.get(track.index()))
            .and_then(|map| map.get(&tick))
            .filter(|info| info.beamed)
            .map(|info| info.group)?;
        self.groups.get(index)
    }

    /// Reconstitue les ligatures d'une piste (portée + voix), mesure après
    /// mesure.
    fn compute_track(&mut self, score: &Score, track: Track, concert_pitch: bool) {
        let staff = track.staff();
        let transpose = if concert_pitch {
            0
        } else {
            score
                .part(score.staff(staff).part)
                .instrument
                .transpose
                .map_or(0, |transpose| transpose.chromatic)
        };

        // État reporté d'une mesure à l'autre, comme le `Beam` persistant de
        // MuseScore : groupe en cours et premier élément d'une ligature à venir.
        let mut groups: Vec<Vec<Element>> = Vec::new();
        let mut open: Option<usize> = None;
        let mut a1: Option<Element> = None;
        let mut prev: Option<Element> = None;

        for (measure_index, measure) in score.measures.iter().enumerate() {
            let elements = measure_elements(score, measure, measure_index, track);
            let time_sig = time_sig_at(score, staff, measure_index, measure);
            let subdivision = beat_subdivision(&elements, measure, &time_sig);

            // Reprise d'une ligature de la mesure précédente (début de mesure).
            if let Some(first) = elements.first() {
                if matches!(
                    first.mode,
                    BeamMode::Mid | BeamMode::End | BeamMode::Begin16 | BeamMode::Begin32
                ) {
                    match &prev {
                        Some(previous)
                            if !matches!(previous.mode, BeamMode::End | BeamMode::None)
                                && !(previous.is_chord() && is_quarter_or_longer(previous)) =>
                        {
                            if open.is_some() {
                                // La ligature continue : `a1` est déjà absorbé.
                                a1 = None;
                            } else {
                                // Une ligature peut commencer sur la mesure
                                // précédente avec un seul élément.
                                a1 = Some(previous.clone());
                            }
                        }
                        _ => {
                            open = None;
                            a1 = None;
                        }
                    }
                } else {
                    open = None;
                    a1 = None;
                }
            }

            for element in elements {
                let bm =
                    actual_beam_mode(&element, prev.as_ref(), measure, &time_sig, &subdivision);
                prev = Some(element.clone());

                let can_be_beamed = stems_hooks(element.duration) > 0 || element.is_rest;
                if !can_be_beamed || bm == BeamMode::None {
                    open = None;
                    a1 = None;
                    continue;
                }

                // Suite d'une ligature en cours.
                if let Some(index) = open {
                    if bm == BeamMode::Begin {
                        // Nouvelle ligature : celle-ci s'arrête ici.
                        open = None;
                    } else {
                        groups[index].push(element.clone());
                        if bm == BeamMode::End {
                            open = None;
                        }
                        continue;
                    }
                }

                // Premier élément d'une ligature potentielle.
                match &a1 {
                    None => a1 = Some(element),
                    Some(start) => {
                        let continues =
                            matches!(bm, BeamMode::Mid | BeamMode::Begin16 | BeamMode::Begin32);
                        let adjacent = start.tick + start.actual >= element.tick;
                        if !continues && (bm == BeamMode::Begin || !adjacent) {
                            a1 = Some(element);
                        } else {
                            let mut group = vec![start.clone()];
                            group.push(element);
                            groups.push(group);
                            open = Some(groups.len() - 1);
                            a1 = None;
                        }
                    }
                }
            }
        }

        // Ligatures complètes : direction de hampe et textes par niveau.
        for group in &groups {
            if group.len() < 2 || group.iter().all(|element| element.is_rest) {
                continue;
            }
            let up = beam_up(score, group, track, concert_pitch, transpose);
            let texts = beam_texts(group);
            let group_index = self.groups.len();
            self.groups.push(BeamGroup {
                start: cr_ref(group.first().expect("groupe non vide")),
                end: cr_ref(group.last().expect("groupe non vide")),
            });
            for (position, element) in group.iter().enumerate() {
                let Some(row) = self
                    .entries
                    .get_mut(element.measure)
                    .and_then(|row| row.get_mut(element.track))
                else {
                    continue;
                };
                row.insert(
                    element.tick,
                    BeamInfo {
                        beamed: true,
                        up,
                        texts: texts[position].clone(),
                        group: group_index,
                    },
                );
            }
        }
    }
}

/// Éléments rythmiques d'une mesure pour une piste, dans l'ordre.
fn measure_elements(
    score: &Score,
    measure: &Measure,
    measure_index: usize,
    track: Track,
) -> Vec<Element> {
    let mut elements = Vec::new();
    for segment in &measure.segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(track) else {
            continue;
        };
        elements.push(Element {
            measure: measure_index,
            track: track.index(),
            tick: segment.tick,
            duration: chord_rest.duration.kind,
            dots: chord_rest.duration.dots,
            actual: global_ticks(score, chord_rest, measure),
            is_rest: chord_rest.is_rest(),
            mode: chord_rest.engraving.beam_mode,
            beam_direction: chord_rest.engraving.beam_direction,
            tuplet: chord_rest.tuplet,
        });
    }
    elements
}

/// Élément rythmique retenu pour le groupement des ligatures.
#[derive(Clone, Debug)]
struct Element {
    measure: usize,
    track: usize,
    tick: Fraction,
    duration: DurationType,
    /// Points de la figure : `TDuration` compare la figure *et* les points
    /// (`durationtype.h:57`), y compris dans la règle de changement de n-olet
    /// de `Groups::baseBeamMode` (`groups.cpp:143`).
    dots: u8,
    actual: Fraction,
    is_rest: bool,
    mode: BeamMode,
    beam_direction: Option<scoretk_core::model::StemDirection>,
    tuplet: Option<scoretk_core::model::TupletId>,
}

/// Vrai si la figure vaut une noire ou plus (`type() <= V_QUARTER`, la
/// comparaison portant sur la valeur brute de l'énumération).
fn is_quarter_or_longer(element: &Element) -> bool {
    matches!(
        element.duration,
        DurationType::Long
            | DurationType::Breve
            | DurationType::Whole
            | DurationType::Half
            | DurationType::Quarter
    )
}

/// Nombre de crochets d'une figure (`TDuration::hooks`), donc de niveaux de
/// ligature (`ChordRest::beams()`).
pub(crate) fn stems_hooks(kind: DurationType) -> u8 {
    match kind {
        DurationType::Long
        | DurationType::Breve
        | DurationType::Whole
        | DurationType::Half
        | DurationType::Quarter
        | DurationType::Zero
        | DurationType::Measure
        | DurationType::Invalid => 0,
        DurationType::Eighth => 1,
        DurationType::Sixteenth => 2,
        DurationType::ThirtySecond => 3,
        DurationType::SixtyFourth => 4,
        DurationType::OneHundredTwentyEighth => 5,
        DurationType::TwoHundredFiftySixth => 6,
        DurationType::FiveHundredTwelfth => 7,
        DurationType::OneThousandTwentyFourth => 8,
    }
}

/// Durée réelle d'un élément, n-olets appliqués (`ChordRest::ticks`).
fn global_ticks(score: &Score, chord_rest: &ChordRest, measure: &Measure) -> Fraction {
    let mut ticks = chord_rest.ticks.unwrap_or_else(|| {
        if chord_rest.is_full_measure_rest() {
            measure.len
        } else {
            chord_rest.duration.ticks()
        }
    });
    let mut tuplet = chord_rest.tuplet;
    let mut depth = 0;
    while let Some(id) = tuplet {
        let Some(value) = score.tuplets.get(id.index()) else {
            break;
        };
        if value.ratio.is_zero() || depth >= 32 {
            break;
        }
        ticks /= value.ratio;
        tuplet = value.parent;
        depth += 1;
    }
    ticks
}

/// Indication de mesure en vigueur sur une portée (`Staff::timeSig`).
fn time_sig_at(
    score: &Score,
    staff: scoretk_core::model::StaffId,
    measure_index: usize,
    measure: &Measure,
) -> TimeSig {
    let track = Track::new(staff, 0);
    let mut found = None;
    for candidate in score.measures.iter().take(measure_index + 1) {
        for segment in &candidate.segments {
            if segment.tick > measure.start_tick {
                break;
            }
            if segment.kind != SegmentType::TimeSig {
                continue;
            }
            if let Some(SegmentItem::TimeSig(timesig)) = segment.element(track) {
                found = Some(timesig.clone());
            }
        }
    }
    found.unwrap_or_else(|| TimeSig::new(4, 4))
}

/// Prépassage par temps des mesures à dénominateur 4 (`createBeams`) :
/// durée la plus courte de chaque temps.
fn beat_subdivision(
    elements: &[Element],
    measure: &Measure,
    time_sig: &TimeSig,
) -> BTreeMap<i64, DurationType> {
    let mut subdivision = BTreeMap::new();
    if time_sig.denominator != 4 {
        return subdivision;
    }
    let stretch = time_sig.stretch;
    for element in elements {
        let rtick = element.tick - measure.start_tick;
        let beat = quarter_floor(rtick * stretch);
        subdivision
            .entry(beat)
            .and_modify(|current: &mut DurationType| {
                if element.duration.ticks() < current.ticks() {
                    *current = element.duration;
                }
            })
            .or_insert(element.duration);
    }
    subdivision
}

/// Partie entière d'une position exprimée en noires.
fn quarter_floor(value: Fraction) -> i64 {
    let quarters = value * Fraction::from_integer(4);
    quarters.numerator() / quarters.denominator()
}

/// Vrai si la position tombe sur un temps (multiple de la noire).
fn on_beat(value: Fraction) -> bool {
    (value * Fraction::from_integer(4)).denominator() == 1
}

/// `Groups::actualBeamMode` : mode effectif après corrections de contexte.
fn actual_beam_mode(
    element: &Element,
    prev: Option<&Element>,
    measure: &Measure,
    time_sig: &TimeSig,
    subdivision: &BTreeMap<i64, DurationType>,
) -> BeamMode {
    if element.is_rest && element.mode == BeamMode::Auto {
        return BeamMode::None;
    }
    if !element.is_rest && stems_hooks(element.duration) == 0 {
        return BeamMode::None;
    }

    let mut mode = base_beam_mode(element, prev, measure, time_sig, None);
    if mode == BeamMode::Auto {
        let rtick = element.tick - measure.start_tick;
        if rtick.is_zero() {
            return BeamMode::Begin;
        }
        let tick = rtick * time_sig.stretch;
        if time_sig.denominator == 4 && on_beat(tick) {
            let beat = quarter_floor(tick);
            let current = subdivision.get(&beat);
            let previous = subdivision.get(&(beat - 1));
            let shortest = match (current, previous) {
                (Some(left), Some(right)) => Some(if left.ticks() <= right.ticks() {
                    *left
                } else {
                    *right
                }),
                (Some(value), None) | (None, Some(value)) => Some(*value),
                (None, None) => None,
            };
            if let Some(shortest) = shortest {
                mode = base_beam_mode(element, prev, measure, time_sig, Some(shortest));
            }
        }
    }
    if mode == BeamMode::Auto {
        BeamMode::Mid
    } else {
        mode
    }
}

/// `Groups::baseBeamMode` : mode de ligature d'après la signature.
///
/// `duration_override` correspond au `setDurationType(minDuration)` temporaire
/// du C++ (même figure et mêmes ticks).
fn base_beam_mode(
    element: &Element,
    prev: Option<&Element>,
    measure: &Measure,
    time_sig: &TimeSig,
    duration_override: Option<DurationType>,
) -> BeamMode {
    if element.mode != BeamMode::Auto {
        return element.mode;
    }

    let duration = duration_override.unwrap_or(element.duration);
    let actual = duration_override.map_or(element.actual, |kind| kind.ticks());
    if !actual.is_positive() {
        return BeamMode::Auto;
    }
    let rtick = element.tick - measure.start_tick;
    let max_tick_len = prev.map_or(actual, |previous| actual.max(previous.actual));

    // Plus petite figure sur laquelle l'élément tombe (croche, double, …).
    let mut smallest = Fraction::new(1, 8);
    let limit = Fraction::new(1, 32);
    loop {
        let divides = (rtick / smallest).denominator() == 1;
        if smallest <= max_tick_len && divides {
            break;
        }
        smallest /= Fraction::from_integer(2);
        if smallest < limit {
            smallest = actual;
            break;
        }
    }

    let big_beat = figure_of(smallest);
    let groups = Groups::of(time_sig);
    let mut tick = rtick * time_sig.stretch;
    // Anacrouse : `Measure::anacrusisOffset`.
    if measure.start_tick.is_zero() && measure.len < measure.nominal_len {
        tick += measure.nominal_len - measure.len;
    }

    let by_type = groups.beam_mode(tick, duration);
    let by_position = groups.beam_mode(tick, big_beat);
    let mut value = if by_type == BeamMode::Auto {
        by_position
    } else {
        by_type
    };

    // Corrections de contexte.
    if value == BeamMode::Auto && !tick.is_zero() {
        if let Some(previous) = prev {
            if element_is_tuplet_change(element, previous) {
                if is_eighth_or_longer(duration) {
                    value = groups.beam_mode(tick, DurationType::Sixteenth);
                } else if duration == DurationType::Sixteenth {
                    value = groups.beam_mode(tick, DurationType::ThirtySecond);
                } else {
                    value = groups.beam_mode(tick, DurationType::SixtyFourth);
                }
            }
        }
        let voice_is_not_first = !element.track.is_multiple_of(VOICES);
        if voice_is_not_first {
            if let Some(previous) = prev {
                let hole = previous.end_tick() < element.tick;
                if !previous.is_tuplet() && hole {
                    value = BeamMode::Begin;
                }
            }
        }
    }

    value
}

/// Vrai si les deux éléments appartiennent à des n-olets différents.
///
/// Le C++ compare des `TDuration` complètes (`groups.cpp:143`), pas seulement
/// `DurationType` : une croche pointée et une croche non pointée sont
/// différentes, ce qui empêche le triolet de Vivaldi 04 de rompre sa ligature.
fn element_is_tuplet_change(element: &Element, previous: &Element) -> bool {
    element.tuplet != previous.tuplet
        && element.duration == previous.duration
        && element.dots == previous.dots
}

fn cr_ref(element: &Element) -> CrRef {
    CrRef {
        measure: element.measure,
        track: Track::from_raw(element.track as u32),
        tick: element.tick,
    }
}

/// Vrai si la figure est une croche ou plus longue (`crDuration >= V_EIGHTH`).
fn is_eighth_or_longer(kind: DurationType) -> bool {
    matches!(
        kind,
        DurationType::Long
            | DurationType::Breve
            | DurationType::Whole
            | DurationType::Half
            | DurationType::Quarter
            | DurationType::Eighth
            | DurationType::Measure
    )
}

impl Element {
    /// Vrai si l'élément est un accord (et non un silence).
    fn is_chord(&self) -> bool {
        !self.is_rest
    }

    /// Vrai si l'élément appartient à un n-olet.
    fn is_tuplet(&self) -> bool {
        self.tuplet.is_some()
    }

    /// Tick de fin réel.
    fn end_tick(&self) -> Fraction {
        self.tick + self.actual
    }
}

/// Groupes de ligature par défaut d'une signature (`Groups::endings`).
struct Groups {
    nodes: Vec<(Fraction, u16)>,
}

impl Groups {
    /// Groupes par défaut de `4/4`, `6/8`, etc., avec le repli du C++ pour les
    /// signatures absentes de la table.
    fn of(time_sig: &TimeSig) -> Self {
        // `<TimeSig><Groups>` du `.mscx` l'emporte sur la table par défaut,
        // comme dans `Staff::group` (`groups.cpp:315-345`).
        if let Some(nodes) = &time_sig.groups {
            return Self {
                nodes: nodes
                    .iter()
                    .map(|(position, action)| (Fraction::new(i64::from(*position), 32), *action))
                    .collect(),
            };
        }
        let signature = (time_sig.numerator, time_sig.denominator);
        if let Some(nodes) = default_groups(signature) {
            return Self {
                nodes: nodes
                    .iter()
                    .map(|(position, action)| (Fraction::new(i64::from(*position), 32), *action))
                    .collect(),
            };
        }

        let unit = match time_sig.denominator {
            2 => Fraction::new(16, 32),
            4 => Fraction::new(8, 32),
            8 => Fraction::new(4, 32),
            16 => Fraction::new(2, 32),
            _ => Fraction::new(1, 32),
        };
        let mut nodes = Vec::new();
        for index in 1..time_sig.numerator {
            nodes.push((unit * Fraction::from_integer(i64::from(index)), 0x111));
        }
        Self { nodes }
    }

    /// `Groups::beamMode` : action sur un niveau pour une position et une
    /// figure données.
    fn beam_mode(&self, tick: Fraction, duration: DurationType) -> BeamMode {
        let shift = match duration {
            DurationType::Eighth => 0,
            DurationType::Sixteenth => 4,
            DurationType::ThirtySecond
            | DurationType::SixtyFourth
            | DurationType::OneHundredTwentyEighth
            | DurationType::TwoHundredFiftySixth
            | DurationType::FiveHundredTwelfth
            | DurationType::OneThousandTwentyFourth => 8,
            _ => return BeamMode::Auto,
        };
        for (position, action) in &self.nodes {
            if *position < tick {
                continue;
            }
            if *position > tick {
                break;
            }
            return match (action >> shift) & 0xf {
                1 => BeamMode::Begin,
                2 => BeamMode::Begin16,
                3 => BeamMode::Begin32,
                _ => BeamMode::Auto,
            };
        }
        BeamMode::Auto
    }
}

/// Table `noteGroups` de `groups.cpp` : positions en trente-deuxièmes de ronde
/// depuis le début de la mesure, actions par niveau (4 bits par niveau :
/// croche, double, triple et plus).
fn default_groups((numerator, denominator): (i32, i32)) -> Option<&'static [(i32, u16)]> {
    let nodes: &'static [(i32, u16)] = match (numerator, denominator) {
        (2, 2) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
        ],
        (3, 2) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
            (32, 0x111),
            (36, 0x200),
            (40, 0x110),
            (44, 0x200),
        ],
        (4, 2) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
            (32, 0x111),
            (36, 0x200),
            (40, 0x110),
            (44, 0x200),
            (48, 0x111),
            (52, 0x200),
            (56, 0x110),
            (60, 0x200),
        ],
        (2, 4) => &[(4, 0x200), (8, 0x111), (12, 0x200)],
        (3, 4) => &[
            (4, 0x200),
            (8, 0x111),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
        ],
        (4, 4) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
        ],
        (5, 4) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x110),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x110),
            (36, 0x200),
        ],
        (6, 4) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x110),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x110),
            (36, 0x200),
            (40, 0x110),
            (44, 0x200),
        ],
        (3, 8) => &[(4, 0x200), (8, 0x200)],
        (5, 8) => &[(4, 0x200), (8, 0x200), (12, 0x111), (16, 0x200)],
        (6, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x200),
        ],
        (7, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x111),
            (24, 0x200),
        ],
        (9, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x200),
        ],
        (12, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x200),
            (36, 0x111),
            (40, 0x200),
            (44, 0x200),
        ],
        _ => return None,
    };
    Some(nodes)
}

/// Figure correspondant à une durée exacte (`TDuration(fraction).type()`).
fn figure_of(value: Fraction) -> DurationType {
    if value >= Fraction::new(1, 4) {
        DurationType::Quarter
    } else if value >= Fraction::new(1, 8) {
        DurationType::Eighth
    } else if value >= Fraction::new(1, 16) {
        DurationType::Sixteenth
    } else {
        DurationType::ThirtySecond
    }
}

/// Direction de hampe d'une ligature (`BeamLayout::layout1`).
fn beam_up(
    score: &Score,
    group: &[Element],
    track: Track,
    concert_pitch: bool,
    transpose: i32,
) -> bool {
    let first = &group[0];
    let measure = &score.measures[first.measure];

    // Direction écrite dans `<Beam><StemDirection>` : elle l'emporte sur tout
    // le reste (`BeamLayout::layout1`, `item->direction() != AUTO`).
    if let Some(direction) = first.beam_direction {
        if direction != scoretk_core::model::StemDirection::Auto {
            return direction == scoretk_core::model::StemDirection::Up;
        }
    }

    // Accords croisés entre portées (`computeUpForMovedCross`, puis
    // `maxCRMove`/`minCRMove`).
    let moves: Vec<i8> = group
        .iter()
        .filter_map(|element| element_chord(score, element).map(|chord| chord.staff_move))
        .collect();
    let max_move = moves.iter().copied().max().unwrap_or(0);
    let min_move = moves.iter().copied().min().unwrap_or(0);
    if max_move > 0 {
        return false;
    }
    if min_move < 0 {
        return true;
    }

    let multiple_voices = stems::has_other_voices(measure, track, first.tick, first.actual);

    if multiple_voices {
        return track.raw().is_multiple_of(2);
    }

    let mut lines = BTreeSet::new();
    let mut has_notes = false;
    for element in group {
        let Some(chord) = element_chord(score, element) else {
            continue;
        };
        has_notes = true;
        if chord.engraving.stem != scoretk_core::model::StemDirection::Auto {
            return chord.engraving.stem == scoretk_core::model::StemDirection::Up;
        }
        for distance in stems::chord_distances(
            score,
            chord,
            element.tick,
            track.staff(),
            concert_pitch,
            transpose,
        ) {
            lines.insert(distance);
        }
    }

    if !has_notes {
        return true;
    }
    let lines: Vec<i32> = lines.into_iter().collect();
    stems::auto_stem_direction(&lines) > 0
}

/// Retrouve l'accord d'un élément de ligature.
fn element_chord<'a>(score: &'a Score, element: &Element) -> Option<&'a Chord> {
    let measure = score.measures.get(element.measure)?;
    let segment = measure
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::ChordRest && segment.tick == element.tick)?;
    let Some(SegmentItem::ChordRest(chord_rest)) =
        segment.element(Track::from_raw(element.track as u32))
    else {
        return None;
    };
    chord_rest.chord_ref()
}

/// Ligature d'un groupe de notes d'ornement.
#[derive(Clone, Debug, Default)]
pub(crate) struct GraceBeam {
    /// Direction de hampe calculée par `BeamLayout::beamGraceNotes`.
    ///
    /// Conservée avec le portage de la disposition ; l'export MusicXML écrit
    /// désormais `xsd:stem` depuis la direction stockée dans le modèle, donc
    /// ce champ n'est plus lu ici.
    #[allow(dead_code)]
    pub up: bool,
    /// Texte des niveaux de cet ornement.
    pub texts: Vec<String>,
}

/// Ligatures des ornements d'un accord (`BeamLayout::beamGraceNotes`).
///
/// `indices` liste les ornements à considérer, dans l'ordre (`before` ou
/// `after`). Les ornements d'une noire ou plus longue ne sont pas ligaturés,
/// et une suite d'au moins deux ornements forme une ligature. La direction
/// d'une ligature d'ornement vaut « haut », sauf sur une portée à plusieurs
/// voix où elle suit la parité de la piste.
pub(crate) fn grace_beams(
    graces: &[scoretk_core::model::GraceChord],
    indices: &[usize],
    multiple_voices: bool,
    track_even: bool,
) -> Vec<Option<GraceBeam>> {
    let mut result = vec![None; indices.len()];
    let mut run: Vec<usize> = Vec::new();

    let flush = |run: &mut Vec<usize>, result: &mut Vec<Option<GraceBeam>>| {
        if run.len() < 2 {
            run.clear();
            return;
        }
        let elements: Vec<Element> = run
            .iter()
            .map(|index| {
                let duration = graces[*index].duration;
                Element {
                    measure: 0,
                    track: 0,
                    tick: Fraction::ZERO,
                    duration: duration.kind,
                    dots: duration.dots,
                    actual: duration.ticks(),
                    is_rest: false,
                    mode: BeamMode::Auto,
                    beam_direction: None,
                    tuplet: None,
                }
            })
            .collect();
        let texts = beam_texts(&elements);
        for (position, index) in run.iter().enumerate() {
            result[*index] = Some(GraceBeam {
                up: if multiple_voices { track_even } else { true },
                texts: texts[position].clone(),
            });
        }
        run.clear();
    };

    for (position, index) in indices.iter().enumerate() {
        let duration = graces[*index].duration;
        if stems_hooks(duration.kind) == 0 || is_quarter_or_longer_duration(duration.kind) {
            flush(&mut run, &mut result);
            continue;
        }
        run.push(position);
    }
    flush(&mut run, &mut result);
    result
}

/// Vrai si la figure vaut une noire ou plus.
fn is_quarter_or_longer_duration(kind: DurationType) -> bool {
    matches!(
        kind,
        DurationType::Long
            | DurationType::Breve
            | DurationType::Whole
            | DurationType::Half
            | DurationType::Quarter
            | DurationType::Measure
    )
}

/// Texte des `<beam>` de chaque élément d'une ligature (`writeBeam`).
fn beam_texts(group: &[Element]) -> Vec<Vec<String>> {
    let mut result = vec![Vec::new(); group.len()];
    for (index, element) in group.iter().enumerate() {
        let levels = stems_hooks(element.duration);
        if levels == 0 {
            continue;
        }
        let bmc = element.mode;
        let bmn = group
            .get(index + 1)
            .map_or(BeamMode::Auto, |value| value.mode);
        let previous = index
            .checked_sub(1)
            .map(|value| stems_hooks(group[value].duration) as i32);
        let next = group
            .get(index + 1)
            .map(|value| stems_hooks(value.duration) as i32);
        let blp = previous.unwrap_or(-1);
        let bln = next.unwrap_or(-1);

        let mut texts = Vec::new();
        for level in 1..=i32::from(levels) {
            let text = if (blp < level && bln >= level)
                || (bmc == BeamMode::Begin16 && level > 1)
                || (bmc == BeamMode::Begin32 && level > 2)
            {
                "begin"
            } else if blp < level && bln < level {
                if bln > 0 {
                    "forward hook"
                } else if blp > 0 {
                    "backward hook"
                } else {
                    continue;
                }
            } else if (blp >= level && bln < level)
                || (bmn == BeamMode::Begin16 && level > 1)
                || (bmn == BeamMode::Begin32 && level > 2)
            {
                "end"
            } else if blp >= level && bln >= level {
                "continue"
            } else {
                continue;
            };
            texts.push(text.to_owned());
        }
        result[index] = texts;
    }
    result
}

/// Élément rythmique et sa position, pour `Beam::calcBeamBreaks`.
#[derive(Clone, Copy)]
pub(crate) struct BeamCr<'a> {
    pub measure: usize,
    pub track: Track,
    pub tick: Fraction,
    pub rest: &'a ChordRest,
}

/// Port de `Beam::calcBeamBreaks` (`engraving/dom/beam.cpp:248-297`).
///
/// Les deux booléens sont `isBroken16` et `isBroken32` (`startChordBreaks32`
/// et `startChordBreaks64` chez l'appelant, `tuplet.cpp:251-267`).
pub(crate) fn calc_beam_breaks(
    score: &Score,
    cr: &BeamCr<'_>,
    prev: &BeamCr<'_>,
    level: i32,
) -> (bool, bool) {
    let element = element_from(score, cr);
    let previous = element_from(score, prev);

    if element.is_rest
        && matches!(
            element.mode,
            BeamMode::Mid | BeamMode::Begin16 | BeamMode::Begin32
        )
    {
        return match element.mode {
            BeamMode::Mid => (false, false),
            BeamMode::Begin16 => (level > 0, false),
            BeamMode::Begin32 => (false, level > 1),
            _ => unreachable!("mode filtré ci-dessus"),
        };
    }

    let measure = &score.measures[element.measure];
    let time_sig = time_sig_at(score, cr.track.staff(), element.measure, measure);
    let default_mode = base_beam_mode(&element, Some(&previous), measure, &time_sig, None);

    let mut broken16 = level >= 1
        && (element.mode == BeamMode::Begin16
            || (element.mode == BeamMode::Auto && default_mode == BeamMode::Begin16));
    let mut broken32 = level >= 2
        && (element.mode == BeamMode::Begin32
            || (element.mode == BeamMode::Auto && default_mode == BeamMode::Begin32));

    // `deal with beam-embedded triplets` (`beam.cpp:275-296`) : les n-olets
    // dont la durée totale vaut moins de `level` crochets rompent le beam.
    if level > 0 && element.mode == BeamMode::Auto && element.tuplet != previous.tuplet {
        let tuplet_id = element.tuplet.or(previous.tuplet);
        if let Some(id) = tuplet_id {
            if let Some(tuplet) = score.tuplets.get(id.index()) {
                let beams = truncated_hooks(tuplet.len).max(1);
                if beams <= level {
                    broken16 = level == 1;
                    broken32 = level >= 2;
                }
            }
        }
    }

    (broken16, broken32)
}

fn element_from(score: &Score, cr: &BeamCr<'_>) -> Element {
    let measure = &score.measures[cr.measure];
    Element {
        measure: cr.measure,
        track: cr.track.index(),
        tick: cr.tick,
        duration: cr.rest.duration.kind,
        dots: cr.rest.duration.dots,
        actual: global_ticks(score, cr.rest, measure),
        is_rest: cr.rest.is_rest(),
        mode: cr.rest.engraving.beam_mode,
        beam_direction: cr.rest.engraving.beam_direction,
        tuplet: cr.rest.tuplet,
    }
}

/// `TDuration(fraction, /* truncate */ true).hooks()` (`durationtype.cpp:406-449`).
fn truncated_hooks(len: Fraction) -> i32 {
    const KINDS: &[DurationType] = &[
        DurationType::Long,
        DurationType::Breve,
        DurationType::Whole,
        DurationType::Half,
        DurationType::Quarter,
        DurationType::Eighth,
        DurationType::Sixteenth,
        DurationType::ThirtySecond,
        DurationType::SixtyFourth,
        DurationType::OneHundredTwentyEighth,
        DurationType::TwoHundredFiftySixth,
        DurationType::FiveHundredTwelfth,
        DurationType::OneThousandTwentyFourth,
    ];
    for kind in KINDS {
        for dots in (0..=4).rev() {
            if Duration::with_dots(*kind, dots).ticks() <= len {
                return i32::from(stems_hooks(*kind));
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groupes_4_4_croches_continues() {
        let groups = Groups::of(&TimeSig::new(4, 4));
        // Sur un temps, une croche continue (pas de rupture à la noire).
        assert_eq!(
            groups.beam_mode(Fraction::new(1, 4), DurationType::Eighth),
            BeamMode::Auto
        );
        // Une double croche rompt au temps.
        assert_eq!(
            groups.beam_mode(Fraction::new(1, 4), DurationType::Sixteenth),
            BeamMode::Begin
        );
        // La première double croche de la mesure démarre aussi.
        assert_eq!(
            groups.beam_mode(Fraction::new(1, 8), DurationType::Sixteenth),
            BeamMode::Auto
        );
    }

    #[test]
    fn groupes_6_8() {
        let groups = Groups::of(&TimeSig::new(6, 8));
        // 6/8 : groupes de trois croches.
        assert_eq!(
            groups.beam_mode(Fraction::new(1, 4), DurationType::Eighth),
            BeamMode::Auto
        );
        assert_eq!(
            groups.beam_mode(Fraction::new(1, 2), DurationType::Eighth),
            BeamMode::Auto
        );
    }

    #[test]
    fn crochets_par_figure() {
        assert_eq!(stems_hooks(DurationType::Quarter), 0);
        assert_eq!(stems_hooks(DurationType::Eighth), 1);
        assert_eq!(stems_hooks(DurationType::Sixteenth), 2);
        assert_eq!(stems_hooks(DurationType::ThirtySecond), 3);
    }

    #[test]
    fn textes_d_une_ligature_simple() {
        let element = |hooks: DurationType| Element {
            measure: 0,
            track: 0,
            tick: Fraction::ZERO,
            duration: hooks,
            dots: 0,
            actual: hooks.ticks(),
            is_rest: false,
            mode: BeamMode::Auto,
            beam_direction: None,
            tuplet: None,
        };
        let group = vec![
            element(DurationType::Eighth),
            element(DurationType::Eighth),
            element(DurationType::Eighth),
        ];
        let texts = beam_texts(&group);
        assert_eq!(
            texts,
            vec![
                vec!["begin".to_owned()],
                vec!["continue".to_owned()],
                vec!["end".to_owned()],
            ]
        );
    }
}
