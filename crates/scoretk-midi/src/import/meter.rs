// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importmidi_meter.cpp` et `importmidi_meter.h` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026, ainsi que de `engraving/dom/durationtype.cpp`
// (`toDurationList`).
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

//! Découpage métrique d'une durée en figures de note.
//!
//! Portage de `Meter` (`importmidi_meter.cpp`) : la mesure est découpée en
//! niveaux métriques (temps, sous-temps, n-olets), et une durée est fractionnée
//! aux points de plus fort accent qu'elle traverse. `toDurationList` vient de
//! `engraving/dom/durationtype.cpp` et choisit la plus grande figure pointée
//! qui tient dans le reste à couvrir.

use std::collections::{BTreeMap, VecDeque};

use mscz2xml::model::{Duration, DurationType, Fraction};

use super::chord::min_allowed_duration;
use super::midifile::{DIVISION, UNITS_PER_WHOLE};
use super::tuplet::{tuplet_limits, TupletData};

/// Niveau d'un bord de n-olet (`TUPLET_BOUNDARY_LEVEL`).
pub const TUPLET_BOUNDARY_LEVEL: i32 = 10;

/// Nature de la durée demandée (`Meter::DurationType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationKind {
    /// Note.
    Note,
    /// Silence.
    Rest,
}

/// Découpage métrique d'un intervalle (`DivisionInfo`).
#[derive(Clone, Debug)]
pub struct DivisionInfo {
    /// Début de l'intervalle.
    pub on_time: Fraction,
    /// Longueur de l'intervalle.
    pub len: Fraction,
    /// Vrai pour un n-olet.
    pub is_tuplet: bool,
    /// Longueurs de division et niveaux associés.
    pub div_lengths: Vec<(Fraction, i32)>,
}

/// Vrai pour une mesure simple (2/2, 3/4, 4/4…) (`isSimple`).
pub fn is_simple(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() < 5
}

/// Vrai pour une mesure composée (6/8, 12/4…) (`isCompound`).
pub fn is_compound(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() % 3 == 0 && bar_fraction.numerator() > 3
}

/// Vrai pour une mesure complexe (5/4, 7/8…) (`isComplex`).
pub fn is_complex(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() == 5 || bar_fraction.numerator() == 7
}

/// Vrai pour une mesure binaire (`isDuple`).
pub fn is_duple(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() == 2 || bar_fraction.numerator() == 6
}

/// Vrai pour une mesure ternaire (`isTriple`).
pub fn is_triple(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() == 3 || bar_fraction.numerator() == 9
}

/// Vrai pour une mesure à quatre temps (`isQuadruple`).
pub fn is_quadruple(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() % 4 == 0
}

/// Vrai pour une mesure à cinq temps (`isQuintuple`).
pub fn is_quintuple(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() % 5 == 0
}

/// Vrai pour une mesure à sept temps (`isSeptuple`).
pub fn is_septuple(bar_fraction: Fraction) -> bool {
    bar_fraction.numerator() % 7 == 0
}

/// Longueur d'un temps (`beatLength`).
pub fn beat_length(bar_fraction: Fraction) -> Fraction {
    if is_duple(bar_fraction) {
        bar_fraction * Fraction::HALF
    } else if is_triple(bar_fraction) {
        bar_fraction / Fraction::from_integer(3)
    } else if is_quadruple(bar_fraction) {
        bar_fraction * Fraction::new(1, 4)
    } else if is_complex(bar_fraction) {
        bar_fraction / Fraction::from_integer(bar_fraction.numerator())
    } else {
        bar_fraction * Fraction::new(1, 4)
    }
}

/// Découpage métrique de la mesure (`metricDivisionsOfBar`).
pub fn metric_divisions_of_bar(bar_fraction: Fraction) -> DivisionInfo {
    let mut info = DivisionInfo {
        on_time: Fraction::ZERO,
        len: bar_fraction,
        is_tuplet: false,
        div_lengths: vec![(bar_fraction, 0)],
    };
    let mut level = 0_i32;
    let push = |info: &mut DivisionInfo, len: Fraction, level: i32| {
        info.div_lengths.push((len, level));
    };
    if is_duple(bar_fraction) {
        level -= 1;
        push(&mut info, bar_fraction * Fraction::HALF, level);
    } else if is_triple(bar_fraction) {
        level -= 1;
        push(&mut info, bar_fraction / Fraction::from_integer(3), level);
    } else if is_quadruple(bar_fraction) {
        level -= 1;
        push(&mut info, bar_fraction * Fraction::HALF, level);
        level -= 1;
        push(&mut info, bar_fraction * Fraction::new(1, 4), level);
    } else if is_quintuple(bar_fraction) {
        level -= 1;
        push(&mut info, bar_fraction / Fraction::from_integer(5), level);
    } else if is_septuple(bar_fraction) {
        level -= 1;
        push(&mut info, bar_fraction / Fraction::from_integer(7), level);
    } else {
        level -= 1;
        push(
            &mut info,
            bar_fraction / Fraction::from_integer(bar_fraction.numerator()),
            level,
        );
    }
    if is_compound(bar_fraction) {
        level -= 1;
        let last = info.div_lengths.last().map(|(len, _)| *len).unwrap_or(bar_fraction);
        level -= 1;
        push(&mut info, last / Fraction::from_integer(3), level);
    }
    while info.div_lengths.last().map(|(len, _)| *len) >= Some(min_allowed_duration() * Fraction::from_integer(2)) {
        let last = info.div_lengths.last().map(|(len, _)| *len).unwrap_or(Fraction::ZERO);
        level -= 1;
        push(&mut info, last * Fraction::HALF, level);
    }
    info
}

/// Découpage métrique d'un n-olet (`metricDivisionsOfTuplet`).
pub fn metric_divisions_of_tuplet(tuplet: &TupletData, mut tuplet_start_level: i32) -> DivisionInfo {
    let mut info = DivisionInfo {
        on_time: tuplet.on_time,
        len: tuplet.len,
        is_tuplet: true,
        div_lengths: vec![(tuplet.len, TUPLET_BOUNDARY_LEVEL)],
    };
    let mut len = tuplet.len / Fraction::from_integer(i64::from(tuplet.tuplet_number));
    tuplet_start_level -= 1;
    info.div_lengths.push((len, tuplet_start_level));
    while len >= min_allowed_duration() * Fraction::from_integer(2) {
        len *= Fraction::HALF;
        tuplet_start_level -= 1;
        info.div_lengths.push((len, tuplet_start_level));
    }
    info
}

/// Longueurs de division de la mesure où un n-olet peut commencer
/// (`divisionsOfBarForTuplets`).
pub fn divisions_of_bar_for_tuplets(bar_fraction: Fraction) -> Vec<Fraction> {
    let info = metric_divisions_of_bar(bar_fraction);
    let beat_len = beat_length(bar_fraction);
    info.div_lengths
        .into_iter()
        .filter(|(len, _)| {
            !((is_compound(bar_fraction) || is_complex(bar_fraction)) && *len > beat_len)
        })
        .map(|(len, _)| len)
        .collect()
}

/// Informations de division d'une mesure, n-olets d'abord
/// (`Meter::divisionInfo`).
pub fn division_info(bar_fraction: Fraction, tuplets_in_bar: &[TupletData]) -> Vec<DivisionInfo> {
    let bar_info = metric_divisions_of_bar(bar_fraction);
    let mut result = Vec::new();
    for tuplet in tuplets_in_bar {
        let mut start_level = 0;
        for (len, level) in &bar_info.div_lengths {
            if *len == tuplet.len {
                start_level = *level;
                break;
            }
        }
        start_level -= 1;
        result.push(metric_divisions_of_tuplet(tuplet, start_level));
    }
    result.push(bar_info);
    result
}

/// Niveau métrique d'un tick compté depuis le début de la mesure
/// (`levelOfTick`).
pub fn level_of_tick(tick: Fraction, divs_info: &[DivisionInfo]) -> i32 {
    for div_info in divs_info {
        if tick < div_info.on_time || tick > div_info.on_time + div_info.len {
            continue;
        }
        for (div_len, level) in &div_info.div_lengths {
            if div_len.is_zero() {
                continue;
            }
            let ratio = (tick - div_info.on_time) / *div_len;
            if ratio.numerator().rem_euclid(ratio.denominator()) == 0 {
                return *level;
            }
        }
    }
    0
}

/// Point de division de plus fort niveau (`MaxLevel`).
#[derive(Clone, Copy, Debug)]
struct MaxLevel {
    pos: Option<Fraction>,
    level: i32,
    level_count: i64,
}

/// Niveau maximal entre deux ticks dans un intervalle (`maxLevelBetween`).
fn max_level_between(start_tick: Fraction, end_tick: Fraction, div_info: &DivisionInfo) -> MaxLevel {
    let mut level = MaxLevel {
        pos: None,
        level: 0,
        level_count: 0,
    };
    let start_in_div = start_tick - div_info.on_time;
    let end_in_div = end_tick - div_info.on_time;
    if start_in_div >= end_in_div
        || start_in_div < Fraction::ZERO
        || end_in_div > div_info.len
    {
        return level;
    }
    for (div_len, div_level) in &div_info.div_lengths {
        if div_len.is_zero() {
            continue;
        }
        let ratio = end_in_div / *div_len;
        let mut max_end_raster =
            *div_len * Fraction::from_integer(ratio.numerator() / ratio.denominator());
        if max_end_raster == end_in_div {
            max_end_raster -= *div_len;
        }
        if start_in_div < max_end_raster {
            let start_ratio = start_in_div / *div_len;
            let max_start_raster =
                *div_len * Fraction::from_integer(start_ratio.numerator() / start_ratio.denominator());
            let count = (max_end_raster - max_start_raster) / *div_len;
            level.pos = Some(max_start_raster + *div_len + div_info.on_time);
            level.level_count = count.numerator() / count.denominator();
            level.level = *div_level;
            break;
        }
    }
    level
}

/// Point de division de plus fort niveau global (`findMaxLevelBetween`).
fn find_max_level_between(
    start_tick: Fraction,
    end_tick: Fraction,
    divs_info: &[DivisionInfo],
) -> MaxLevel {
    let mut level = MaxLevel {
        pos: None,
        level: 0,
        level_count: 0,
    };
    for div_info in divs_info {
        if div_info.is_tuplet {
            if start_tick < div_info.on_time + div_info.len
                && end_tick > div_info.on_time + div_info.len
            {
                level.level = TUPLET_BOUNDARY_LEVEL;
                level.level_count = 1;
                level.pos = Some(div_info.on_time + div_info.len);
                break;
            }
            if start_tick < div_info.on_time
                && end_tick > div_info.on_time
                && end_tick <= div_info.on_time + div_info.len
            {
                level.level = TUPLET_BOUNDARY_LEVEL;
                level.level_count = 1;
                level.pos = Some(div_info.on_time);
                break;
            }
            if start_tick >= div_info.on_time && end_tick <= div_info.on_time + div_info.len {
                level = max_level_between(start_tick, end_tick, div_info);
                break;
            }
        } else {
            level = max_level_between(start_tick, end_tick, div_info);
            break;
        }
    }
    level
}

/// Vrai si une durée est une figure binaire simple (`isSimpleNoteDuration`).
pub fn is_simple_note_duration(duration: Fraction) -> bool {
    if duration.is_zero() {
        return false;
    }
    let division = Fraction::from_ticks(DIVISION, UNITS_PER_WHOLE as u32);
    let div = if duration > division {
        duration / division
    } else {
        division / duration
    };
    if !div.is_positive() {
        return false;
    }
    let min_val = div.numerator().min(div.denominator());
    let max_val = div.numerator().max(div.denominator());
    min_val == 1 && max_val.is_power_of_two()
}

/// Vrai si la fin des deux tiers d'un temps composé est un silence
/// (`is23EndOfBeatInCompoundMeter`).
fn is_23_end_of_beat_in_compound_meter(
    start_tick: Fraction,
    end_tick: Fraction,
    bar_fraction: Fraction,
) -> bool {
    if end_tick <= start_tick || !is_compound(bar_fraction) {
        return false;
    }
    let beat_len = beat_length(bar_fraction);
    let div_len = beat_len / Fraction::from_integer(3);
    let ratio1 = start_tick / beat_len;
    let ratio2 = end_tick / beat_len;
    (start_tick - beat_len * Fraction::from_integer(ratio1.numerator() / ratio1.denominator())
        == div_len)
        && ratio2
            .numerator()
            .rem_euclid(ratio2.denominator())
            == 0
}

/// Vrai si un silence occupe deux tiers d'une mesure ternaire
/// (`is2of3RestInTripleMeter`).
fn is_2of3_rest_in_triple_meter(
    start_tick: Fraction,
    end_tick: Fraction,
    bar_fraction: Fraction,
) -> bool {
    if end_tick - start_tick <= Fraction::ZERO {
        return false;
    }
    is_triple(bar_fraction)
        && (start_tick == Fraction::ZERO || end_tick == bar_fraction)
        && end_tick - start_tick == (bar_fraction * Fraction::from_integer(2)) / Fraction::from_integer(3)
}

/// Nœud du découpage (`Node`).
#[derive(Clone, Copy, Debug)]
struct Node {
    edge_level: i32,
    mid_level: i32,
}

/// Convertit une longueur en suite de figures, la plus grande d'abord
/// (`toDurationList` de `durationtype.cpp`).
///
/// `max_dots` limite les points de prolongation (1 dans `Meter`, 4 ailleurs
/// comme `engraving::toDurationList`).
pub fn to_duration_list(
    mut length: Fraction,
    use_dots: bool,
    max_dots: u8,
    _print_rest_remains: bool,
) -> Vec<Duration> {
    let max_dots = if use_dots { max_dots } else { 0 };
    let mut result = Vec::new();

    // Figures de la plus longue à la plus courte, comme `DurationType`.
    const TYPES: [DurationType; 14] = [
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
        DurationType::Zero,
    ];

    while length.is_positive() {
        let mut found = None;
        for kind in TYPES {
            if matches!(kind, DurationType::Zero) {
                break;
            }
            let mut dots = i32::from(max_dots);
            while dots >= 0 {
                let duration = Duration::with_dots(kind, dots as u8);
                let fraction = duration.ticks();
                if fraction <= length {
                    found = Some(duration);
                    break;
                }
                dots -= 1;
            }
            if found.is_some() {
                break;
            }
        }
        match found {
            Some(duration) => {
                length -= duration.ticks();
                result.push(duration);
            }
            None => break,
        }
    }
    result
}

/// Nombre de figures nécessaires (`noteCount`).
fn note_count(duration: Fraction, use_dots: bool) -> usize {
    to_duration_list(duration, use_dots, 1, false).len()
}

/// Vrai si découper en deux coûte plus de figures que garder le nœud
/// (`isLessNoteCount`).
fn is_less_note_count(t1: Fraction, t2: Fraction, t3: Fraction, use_dots: bool) -> bool {
    note_count(t3 - t1, use_dots) < note_count(t2 - t1, use_dots) + note_count(t3 - t2, use_dots)
}

/// Vrai si le niveau du nœud central est trop éloigné des bords
/// (`badLevelCondition`).
fn bad_level_condition(start_level_diff: i32, end_level_diff: i32, tol: i32) -> bool {
    start_level_diff > tol || end_level_diff > tol
}

/// Retire les nœuds intermédiaires inutiles (`excludeNodes`).
fn exclude_nodes(nodes: &mut BTreeMap<Fraction, Node>, tol: i32, use_dots: bool) {
    if tol == 0 {
        return;
    }
    loop {
        let positions: Vec<Fraction> = nodes.keys().copied().collect();
        if positions.len() < 3 {
            return;
        }
        let mut removed = false;
        let mut index = 0;
        while index + 2 < positions.len() {
            let p1 = nodes[&positions[index]];
            let p2 = nodes[&positions[index + 1]];
            let p3 = nodes[&positions[index + 2]];
            if !bad_level_condition(p2.mid_level - p1.edge_level, p2.mid_level - p3.edge_level, tol)
                && is_less_note_count(
                    positions[index],
                    positions[index + 1],
                    positions[index + 2],
                    use_dots,
                )
            {
                nodes.remove(&positions[index + 1]);
                removed = true;
                break;
            }
            index += 1;
        }
        if !removed {
            return;
        }
    }
}

/// Niveau de bord ajusté après un n-olet (`adjustEdgeLevelIfTuplet`).
fn adjust_edge_level_if_tuplet(split_point: &MaxLevel, div_info: &[DivisionInfo]) -> i32 {
    if split_point.level == TUPLET_BOUNDARY_LEVEL {
        if let Some(pos) = split_point.pos {
            if let Some(bar_info) = div_info.last() {
                return level_of_tick(pos, std::slice::from_ref(bar_info));
            }
        }
    }
    split_point.level
}

/// Convertit une durée en suite de figures et de ratios de n-olet
/// (`Meter::toDurationList`).
///
/// Renvoie des couples `(ratio de n-olet, figure)` : pour une durée hors
/// n-olet, le ratio vaut `1/1`.
pub fn meter_to_duration_list(
    start_tick_in_bar: Fraction,
    end_tick_in_bar: Fraction,
    bar_fraction: Fraction,
    tuplets_in_bar: &[TupletData],
    kind: DurationKind,
    use_dots: bool,
) -> Vec<(Fraction, Duration)> {
    if start_tick_in_bar < Fraction::ZERO
        || end_tick_in_bar <= start_tick_in_bar
        || end_tick_in_bar > bar_fraction
    {
        return Vec::new();
    }
    let div_info = division_info(bar_fraction, tuplets_in_bar);
    let min_duration = min_allowed_duration() * Fraction::from_integer(2);

    let mut nodes: BTreeMap<Fraction, Node> = BTreeMap::new();
    let start_level = level_of_tick(start_tick_in_bar, &div_info);
    nodes.insert(
        start_tick_in_bar,
        Node {
            edge_level: start_level,
            mid_level: start_level,
        },
    );
    let end_level = level_of_tick(end_tick_in_bar, &div_info);
    nodes.insert(
        end_tick_in_bar,
        Node {
            edge_level: end_level,
            mid_level: end_level,
        },
    );

    let mut gaps: VecDeque<(Fraction, Fraction)> = VecDeque::new();
    gaps.push_back((start_tick_in_bar, end_tick_in_bar));

    while let Some((first, second)) = gaps.pop_front() {
        if second - first < min_duration {
            continue;
        }
        let split_point = find_max_level_between(first, second, &div_info);
        let Some(split_pos) = split_point.pos else {
            continue;
        };
        let effective_level = split_point.level + (split_point.level_count as i32) - 1;
        let Some(start_node) = nodes.get(&first).copied() else {
            continue;
        };
        let Some(end_node) = nodes.get(&second).copied() else {
            continue;
        };
        if bad_level_condition(effective_level - start_node.edge_level, effective_level - end_node.edge_level, 0)
            || is_2of3_rest_in_triple_meter(first, second, bar_fraction)
            || (kind == DurationKind::Rest
                && is_23_end_of_beat_in_compound_meter(first, second, bar_fraction))
        {
            let edge_level = adjust_edge_level_if_tuplet(&split_point, &div_info);
            nodes.insert(
                split_pos,
                Node {
                    edge_level,
                    mid_level: effective_level,
                },
            );
            gaps.push_back((first, split_pos));
            gaps.push_back((split_pos, second));
        }
    }

    let tol = if kind == DurationKind::Note { 1 } else { 0 };
    exclude_nodes(&mut nodes, tol, use_dots);

    // `collectDurations`.
    let mut result = Vec::new();
    let positions: Vec<Fraction> = nodes.keys().copied().collect();
    for window in positions.windows(2) {
        let (first, second) = (window[0], window[1]);
        let tuplet_number = tuplets_in_bar.iter().find_map(|tuplet| {
            (first >= tuplet.on_time && second <= tuplet.on_time + tuplet.len)
                .then_some(tuplet.tuplet_number)
        });
        let ratio = match tuplet_number {
            Some(number) => tuplet_limits(number).ratio,
            None => Fraction::ONE,
        };
        let duration = ratio * (second - first);
        for figure in to_duration_list(duration, use_dots, 1, true) {
            result.push((ratio, figure));
        }
    }
    result
}
