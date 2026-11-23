// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/check.cpp`, `engraving/dom/durationtype.cpp` et
// `engraving/dom/sig.cpp` de MuseScore Studio, Copyright (C) MuseScore
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

//! Remplissage des trous d'une voix par des silences « gap ».
//!
//! Portage de `Measure::checkMeasure` et `Measure::fillGap`
//! (`engraving/dom/check.cpp`) : après lecture, MuseScore parcourt chaque voix
//! et insère des silences invisibles (`Rest::isGap`) dans les intervalles
//! laissés vides. Ces silences ne sont pas exportés, mais leur position fait
//! avancer le curseur d'export : c'est l'origine des `<forward>`/`<backup>`
//! scindés de MuseScore.
//!
//! Les durées des silences viennent de `toRhythmicDurationList`
//! (`engraving/dom/durationtype.cpp`), qui découpe un trou sur les temps et
//! sous-temps forts de l'indication de mesure.

use crate::model::{
    ChordRest, ChordRestKind, DurationType, Fraction, Rest, Score, Segment, SegmentItem,
    SegmentType, Tuplet, VOICES,
};

/// Unités par ronde, comme `4 * Constants::DIVISION` chez MuseScore.
const UNITS: i64 = 4 * 480;

/// Indication de mesure nominale (numérateur / dénominateur).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sig {
    numerator: i64,
    denominator: i64,
}

/// Niveau de temps (`BeatType` dans `engraving/dom/sig.h`).
///
/// L'ordre des variantes est significatif : les comparaisons du C++ reposent
/// sur leur valeur numérique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BeatType {
    Downbeat,
    CompoundStressed,
    SimpleStressed,
    CompoundUnstressed,
    SimpleUnstressed,
    CompoundSubbeat,
    Subbeat,
}

impl Sig {
    fn is_compound(self) -> bool {
        self.numerator > 3 && self.numerator % 3 == 0
    }

    fn d_unit_ticks(self) -> i64 {
        UNITS / self.denominator
    }

    fn d_units_per_beat(self) -> i64 {
        if self.is_compound() {
            3
        } else {
            1
        }
    }

    fn beat_ticks(self) -> i64 {
        self.d_unit_ticks() * self.d_units_per_beat()
    }

    fn beats_per_measure(self) -> i64 {
        self.numerator / self.d_units_per_beat()
    }

    fn is_triple(self) -> bool {
        self.beats_per_measure() % 3 == 0
    }

    fn is_duple(self) -> bool {
        !self.is_triple() && self.beats_per_measure() % 2 == 0
    }

    fn subbeat_ticks(self, level: i64) -> i64 {
        self.d_unit_ticks() >> level
    }

    fn ticks_past_d_unit(self, rtick: i64) -> i64 {
        rtick % self.d_unit_ticks()
    }

    fn ticks_to_next_d_unit(self, rtick: i64) -> i64 {
        self.d_unit_ticks() - self.ticks_past_d_unit(rtick)
    }

    fn ticks_past_beat(self, rtick: i64) -> i64 {
        rtick % self.beat_ticks()
    }

    fn ticks_to_next_beat(self, rtick: i64) -> i64 {
        self.beat_ticks() - self.ticks_past_beat(rtick)
    }

    fn ticks_past_subbeat(self, rtick: i64, level: i64) -> i64 {
        rtick % self.subbeat_ticks(level)
    }

    fn ticks_to_next_subbeat(self, rtick: i64, level: i64) -> i64 {
        self.subbeat_ticks(level) - self.ticks_past_subbeat(rtick, level)
    }

    fn rtick2beat_type(self, rtick: i64) -> BeatType {
        if rtick == 0 {
            return BeatType::Downbeat;
        }
        if rtick % self.d_unit_ticks() != 0 {
            return BeatType::Subbeat;
        }
        if self.is_compound() && rtick % self.beat_ticks() != 0 {
            return BeatType::CompoundSubbeat;
        }
        let beat_num = rtick / self.beat_ticks();
        let stress_beat = if self.is_triple() {
            3
        } else if self.is_duple() {
            2
        } else {
            (self.numerator + 1) / 2
        };
        if stress_beat != 0 && beat_num % stress_beat == 0 {
            return if self.is_compound() {
                BeatType::CompoundStressed
            } else {
                BeatType::SimpleStressed
            };
        }
        if self.is_compound() {
            BeatType::CompoundUnstressed
        } else {
            BeatType::SimpleUnstressed
        }
    }

    fn rtick2subbeat_level(self, rtick: i64) -> i64 {
        let mut level = 0;
        let mut subbeat_ticks = self.d_unit_ticks();
        let mut remainder = rtick % subbeat_ticks;
        while remainder != 0 {
            level += 1;
            if subbeat_ticks % 2 != 0 {
                return -level;
            }
            subbeat_ticks /= 2;
            remainder %= subbeat_ticks;
        }
        level
    }

    fn strongest_subbeat_level_in_range(self, rtick1: i64, rtick2: i64, split: &mut i64) -> i64 {
        let mut level = 0;
        let mut subbeat_ticks = self.d_unit_ticks();
        loop {
            let n = rtick1 / subbeat_ticks;
            let m = (rtick2 - 1) / subbeat_ticks;
            if m > n {
                *split = m * subbeat_ticks;
                return level;
            }
            level += 1;
            if subbeat_ticks % 2 != 0 {
                return -level;
            }
            subbeat_ticks /= 2;
        }
    }

    fn strongest_beat_in_range(
        self,
        rtick1: i64,
        rtick2: i64,
        units_crossed: &mut i64,
        split: &mut i64,
        save_last: bool,
    ) -> BeatType {
        let mut strongest = BeatType::Subbeat;
        let mut rtick = rtick1 + self.ticks_to_next_d_unit(rtick1);
        while rtick < rtick2 {
            *units_crossed += 1;
            let kind = self.rtick2beat_type(rtick);
            let threshold = strongest as i32 + i32::from(save_last);
            if (kind as i32) < threshold {
                strongest = kind;
                *split = rtick;
            }
            rtick += self.d_unit_ticks();
        }
        strongest
    }
}

/// Plus grande figure (sans point) qui tient dans `length`.
fn largest_fitting(length: Fraction) -> Option<DurationType> {
    const TYPES: [DurationType; 12] = [
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
    ];
    TYPES
        .into_iter()
        .find(|kind| kind.ticks() <= length && !kind.ticks().is_zero())
}

/// `toDurationList` : plus grandes figures successives, sans point.
fn to_duration_list(length: Fraction) -> Vec<DurationType> {
    crate::duration_list::to_duration_list(length, false, 0, false)
        .into_iter()
        .map(|duration| duration.kind)
        .collect()
}

/// `toRhythmicDurationList` : découpe un trou sur les temps forts.
fn to_rhythmic_duration_list(
    length: Fraction,
    rtick_start: Fraction,
    sig: Sig,
    measure_len: Fraction,
) -> Vec<DurationType> {
    if !length.is_positive() {
        return Vec::new();
    }
    // Un silence qui occupe toute la mesure reste une mesure entière.
    if length == measure_len {
        return vec![DurationType::Measure];
    }
    let (Some(units), Some(start_units)) = (units_of(length), units_of(rtick_start)) else {
        return to_duration_list(length);
    };
    let mut list = Vec::new();
    if sig.is_compound() {
        split_compound_beats(&mut list, units, start_units, sig);
    } else {
        populate(&mut list, units, start_units, sig);
    }
    list
}

/// Convertit une fraction de ronde en unités MuseScore (1/1920 de ronde).
fn units_of(value: Fraction) -> Option<i64> {
    let scaled = value * Fraction::from_integer(UNITS);
    if scaled.denominator() != 1 {
        return None;
    }
    Some(scaled.numerator())
}

/// `splitCompoundBeatsForList`.
fn split_compound_beats(list: &mut Vec<DurationType>, units: i64, start: i64, sig: Sig) {
    let end = start + units;
    let start_beat = sig.rtick2beat_type(start);
    let end_beat = sig.rtick2beat_type(end);
    if start_beat > BeatType::CompoundUnstressed {
        let split = sig.ticks_to_next_beat(start);
        if units > split {
            let left = split;
            populate(list, left, start, sig);
            split_compound_beats(list, units - left, start + left, sig);
            return;
        }
    }
    if end_beat > BeatType::CompoundUnstressed {
        let split = sig.ticks_past_beat(end);
        if units > split {
            let right = split;
            let left = units - right;
            populate(list, left, start, sig);
            populate(list, right, end - split, sig);
            return;
        }
    }
    populate(list, units, start, sig);
}

/// `populateRhythmicList` : découpe récursivement sur le temps fort traversé.
fn populate(list: &mut Vec<DurationType>, units: i64, start: i64, sig: Sig) {
    let end = start + units;
    let mut need_to_split = false;
    let mut split = 0i64;

    let start_level = sig.rtick2subbeat_level(start);
    let end_level = sig.rtick2subbeat_level(end);
    let strongest_crossed = sig.strongest_subbeat_level_in_range(start, end, &mut split);

    if start_level < 0 || end_level < 0 || strongest_crossed < 0 {
        list.extend(to_duration_list(units_fraction(units)));
        return;
    }
    if strongest_crossed < start_level && strongest_crossed < end_level {
        need_to_split = true;
    }
    if start_level == end_level && strongest_crossed == start_level - 1 {
        need_to_split = false;
    }
    if start_level == end_level && strongest_crossed == start_level - 2 {
        let to_next = sig.ticks_to_next_subbeat(start, start_level - 1);
        let past_prev = sig.ticks_past_subbeat(start, start_level - 1);
        need_to_split = to_next != past_prev;
    }

    if !need_to_split && strongest_crossed == 0 {
        let start_beat = sig.rtick2beat_type(start);
        let end_beat = sig.rtick2beat_type(end);
        let mut units_crossed = 0;
        let use_last = start_beat <= BeatType::SimpleUnstressed;
        let strongest_beat =
            sig.strongest_beat_in_range(start, end, &mut units_crossed, &mut split, use_last);
        need_to_split = force_rhythmic_split(
            true,
            start_beat,
            end_beat,
            units_crossed,
            strongest_beat,
            sig,
        );
    }

    if !need_to_split {
        let length = units_fraction(units);
        if let Some(kind) = largest_fitting(length).filter(|kind| kind.ticks() == length) {
            list.push(kind);
            return;
        }
    }

    if start >= split || split >= end {
        list.extend(to_duration_list(units_fraction(units)));
        return;
    }

    let left = split - start;
    populate(list, left, start, sig);
    populate(list, units - left, split, sig);
}

/// `forceRhythmicSplit`.
fn force_rhythmic_split(
    is_rest: bool,
    start_beat: BeatType,
    end_beat: BeatType,
    units_crossed: i64,
    strongest: BeatType,
    sig: Sig,
) -> bool {
    if strongest <= BeatType::SimpleStressed && !sig.is_triple() && !sig.is_duple() {
        return true;
    }
    if is_rest {
        if strongest <= BeatType::SimpleUnstressed && sig.numerator == 2 {
            return true;
        }
        if strongest <= BeatType::SimpleUnstressed && sig.is_triple() {
            return true;
        }
    }
    if sig.is_compound() {
        force_rhythmic_split_compound(is_rest, start_beat, end_beat, units_crossed, strongest)
    } else {
        force_rhythmic_split_simple(is_rest, start_beat, end_beat, units_crossed, strongest)
    }
}

fn force_rhythmic_split_compound(
    is_rest: bool,
    start_beat: BeatType,
    end_beat: BeatType,
    units_crossed: i64,
    strongest: BeatType,
) -> bool {
    match strongest {
        BeatType::CompoundStressed => force_rhythmic_split_simple(
            is_rest,
            start_beat,
            end_beat,
            units_crossed / 3,
            BeatType::SimpleStressed,
        ),
        BeatType::CompoundUnstressed => false,
        BeatType::CompoundSubbeat => {
            if start_beat <= BeatType::CompoundUnstressed
                && end_beat <= BeatType::CompoundUnstressed
            {
                return false;
            }
            if is_rest && start_beat > BeatType::CompoundUnstressed {
                return true;
            }
            force_rhythmic_split_simple(
                is_rest,
                start_beat,
                end_beat,
                units_crossed,
                BeatType::SimpleUnstressed,
            )
        }
        _ => force_rhythmic_split_simple(is_rest, start_beat, end_beat, units_crossed, strongest),
    }
}

fn force_rhythmic_split_simple(
    is_rest: bool,
    start_beat: BeatType,
    end_beat: BeatType,
    beats_crossed: i64,
    strongest: BeatType,
) -> bool {
    match strongest {
        BeatType::SimpleStressed => {
            if is_rest {
                return true;
            }
            if start_beat <= BeatType::SimpleStressed || end_beat <= BeatType::SimpleStressed {
                return false;
            }
            if start_beat <= BeatType::SimpleUnstressed && end_beat <= BeatType::SimpleUnstressed {
                return false;
            }
            true
        }
        BeatType::SimpleUnstressed => {
            if start_beat <= BeatType::SimpleStressed && end_beat <= BeatType::SimpleStressed {
                return false;
            }
            if start_beat == BeatType::Subbeat || end_beat == BeatType::Subbeat {
                return is_rest || beats_crossed > 1;
            }
            false
        }
        _ => false,
    }
}

/// Fraction correspondant à un nombre d'unités.
fn units_fraction(units: i64) -> Fraction {
    Fraction::new(units, UNITS)
}

/// Remplit les trous de toutes les voix de toutes les mesures.
pub fn fill(score: &mut Score) {
    let count = score.measures.len();
    for index in 0..count {
        if score
            .staves
            .iter()
            .any(|staff| staff.time_stretch != Fraction::ONE)
        {
            // L'étirement local (`Staff::timeStretch`) n'est pas reproduit ici.
            return;
        }
        let sig = timesig_at(score, index);
        let staff_count = score.staves.len();
        for staff in 0..staff_count {
            fill_staff(score, index, staff, sig);
        }
    }
}

/// Indication de mesure en vigueur au début d'une mesure (héritée au besoin).
fn timesig_at(score: &Score, index: usize) -> Sig {
    let fallback = Sig {
        numerator: 4,
        denominator: 4,
    };
    for candidate in score.measures[..=index].iter().rev() {
        for segment in candidate.segments.iter().rev() {
            if segment.kind != SegmentType::TimeSig {
                continue;
            }
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::TimeSig(time_sig) = item {
                    if time_sig.numerator > 0 && time_sig.denominator > 0 {
                        return Sig {
                            numerator: i64::from(time_sig.numerator),
                            denominator: i64::from(time_sig.denominator),
                        };
                    }
                }
            }
        }
        // Ne pas s'arrêter à la mesure précédente : l'indication peut avoir
        // été écrite dans une mesure bien antérieure et rester en vigueur
        // (`TimeSigMap` du C++). Cas réel : douze portées de Douce nuit où
        // seul le premier `<Measure>` porte `<sigN>6</sigN>`.
    }
    fallback
}

/// Remplit les trous d'une portée sur une mesure (`Measure::checkMeasure`).
fn fill_staff(score: &mut Score, index: usize, staff: usize, sig: Sig) {
    let Some(measure) = score.measures.get(index) else {
        return;
    };
    let measure_len = measure.len;
    let start_tick = measure.start_tick;
    let staff_id = score.staves[staff].id;
    // `Measure::hasVoices` (`measure.cpp:2198-2262`, appelé par
    // `checkMeasure`) ne compte que les ChordRest **visibles** dans les voix
    // 2..4 : un silence invisible n'y crée pas de voix, donc pas de silences
    // gap et pas de `<forward>` surnuméraires à l'export.
    let has_voices = (1..VOICES as u8).any(|voice| {
        measure.segments.iter().any(|segment| {
            if segment.kind != SegmentType::ChordRest {
                return false;
            }
            segment
                .chord_rest(crate::model::Track::new(staff_id, voice))
                .is_some_and(|cr| match &cr.kind {
                    ChordRestKind::Rest(rest) => rest.visible && !rest.gap,
                    ChordRestKind::Chord(_) => true,
                })
        })
    });
    let tracks = if has_voices { VOICES } else { 1 };
    let mut gaps: Vec<(Fraction, u8, DurationType, bool)> = Vec::new();

    for voice in 0..tracks {
        let track = crate::model::Track::new(staff_id, voice as u8);
        let mut expected = Fraction::ZERO;
        let mut skip_until = Fraction::ZERO;
        for segment in &measure.segments {
            if segment.kind != SegmentType::ChordRest {
                continue;
            }
            let Some(chord_rest) = segment.chord_rest(track) else {
                continue;
            };
            let current = segment.tick - start_tick;
            if current < skip_until {
                continue;
            }
            if current < expected {
                break;
            }
            if current > expected {
                push_gap(
                    &mut gaps,
                    voice as u8,
                    expected,
                    current - expected,
                    sig,
                    measure_len,
                );
                if current >= measure_len {
                    break;
                }
            }
            let Some(tuplet) = top_tuplet(score, chord_rest) else {
                let ticks = chord_rest
                    .ticks
                    .unwrap_or_else(|| chord_rest.ticks_in_measure(measure_len));
                expected = current + ticks;
                continue;
            };
            let end = tuplet.tick - start_tick + tuplet.len;
            skip_until = end;
            expected = end;
        }
        if measure_len > expected && (!expected.is_zero() || voice == 0) {
            push_gap(
                &mut gaps,
                voice as u8,
                expected,
                measure_len - expected,
                sig,
                measure_len,
            );
        }
    }

    for (position, voice, kind, full_measure) in gaps {
        add_gap_rest(score, index, staff, voice, position, kind, full_measure);
    }
}

/// Ajoute les durées d'un trou à la liste, sur les temps forts.
fn push_gap(
    gaps: &mut Vec<(Fraction, u8, DurationType, bool)>,
    voice: u8,
    mut start: Fraction,
    length: Fraction,
    sig: Sig,
    measure_len: Fraction,
) {
    for kind in to_rhythmic_duration_list(length, start, sig, measure_len) {
        let full_measure = kind == DurationType::Measure;
        gaps.push((start, voice, kind, full_measure));
        start += if full_measure {
            measure_len
        } else {
            kind.ticks()
        };
    }
}

/// N-olet le plus externe d'un élément (`ChordRest::topTuplet`).
fn top_tuplet<'a>(score: &'a Score, chord_rest: &ChordRest) -> Option<&'a Tuplet> {
    let mut tuplet = chord_rest
        .tuplet
        .and_then(|id| score.tuplets.get(id.index()));
    while let Some(parent) = tuplet.and_then(|value| value.parent) {
        tuplet = score.tuplets.get(parent.index());
    }
    tuplet
}

/// Insère un silence « gap » dans la mesure (`fillGap`).
fn add_gap_rest(
    score: &mut Score,
    index: usize,
    staff: usize,
    voice: u8,
    position: Fraction,
    kind: DurationType,
    full_measure: bool,
) {
    let Some(measure) = score.measures.get_mut(index) else {
        return;
    };
    let tick = measure.start_tick + position;
    let staff_id = score.staves[staff].id;
    if !measure
        .segments
        .iter()
        .any(|segment| segment.kind == SegmentType::ChordRest && segment.tick == tick)
    {
        let segment = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
        measure.add_segment(segment);
    }
    let Some(segment_index) = measure
        .segments
        .iter()
        .position(|segment| segment.kind == SegmentType::ChordRest && segment.tick == tick)
    else {
        return;
    };
    let rest = ChordRest {
        duration: if full_measure {
            crate::model::Duration::measure()
        } else {
            crate::model::Duration::new(kind)
        },
        ticks: if full_measure {
            None
        } else {
            Some(kind.ticks())
        },
        tuplet: None,
        lyrics: Vec::new(),
        beam_mode: crate::model::BeamMode::Auto,
        beam_direction: None,
        kind: crate::model::ChordRestKind::Rest(Rest {
            full_measure,
            gap: true,
            visible: true,
            note_dots: Vec::new(),
        }),
    };
    if let Some(segment) = measure.segments.get_mut(segment_index) {
        segment.set_element(
            crate::model::Track::new(staff_id, voice),
            SegmentItem::ChordRest(rest),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La mesure 45 de Douce nuit est en 6/8, mais l'indication est portée
    /// par la mesure 0 : la recherche doit remonter jusqu'à elle. Le portage
    /// s'arrêtait à la mesure précédente et retombait sur 4/4, changeant la
    /// segmentation des silences gap (et les `<forward>` de l'export).
    /// Tarentella Napoletana, mesure 101 : la voix 2 ne contient qu'un
    /// silence invisible (`<visible>0</visible>`). `Measure::hasVoices`
    /// (`measure.cpp:2198-2262`) ne compte pas les éléments invisibles : la
    /// voix n'est donc pas remplie de silences gap, et l'export n'écrit
    /// aucun `<forward>` surnuméraire.
    #[test]
    fn voix_avec_silence_invisible_nest_pas_remplie() {
        let mut score = crate::model::Score::new();
        let part = score.add_part(crate::model::Part::new("P", "P"));
        let staff = score.add_staff(part);
        let mut measure =
            crate::model::Measure::new(0, crate::model::Tick::ZERO, Fraction::new(3, 4));
        for voice in 0..2 {
            let mut segment = crate::model::Segment::new(
                crate::model::SegmentType::ChordRest,
                crate::model::Tick::ZERO,
                score.staves.len(),
            );
            let mut rest = crate::model::ChordRest::rest(crate::model::Duration::quarter());
            if let crate::model::ChordRestKind::Rest(r) = &mut rest.kind {
                r.visible = voice == 0;
            }
            segment.set_element(
                crate::model::Track::new(staff, voice),
                crate::model::SegmentItem::ChordRest(rest),
            );
            measure.add_segment(segment);
        }
        score.add_measure(measure);

        fill(&mut score);
        let gaps: usize = score.measures[0]
            .segments
            .iter()
            .flat_map(|segment| {
                segment
                    .chord_rest(crate::model::Track::new(staff, 1))
                    .and_then(|cr| cr.rest_ref())
                    .filter(|rest| rest.gap)
                    .map(|_| 1)
            })
            .sum();
        assert_eq!(gaps, 0, "silences gap créés dans une voix invisible");
    }

    #[test]
    fn indication_de_mesure_heritee_dune_mesure_anterior() {
        let mut score = crate::model::Score::new();
        let part = score.add_part(crate::model::Part::new("P", "P"));
        let staff = score.add_staff(part);
        let mut first = crate::model::Measure::new(0, crate::model::Tick::ZERO, Fraction::ONE);
        let mut segment = crate::model::Segment::new(
            crate::model::SegmentType::TimeSig,
            crate::model::Tick::ZERO,
            score.staves.len(),
        );
        segment.set_element(
            crate::model::Track::new(staff, 0),
            crate::model::SegmentItem::TimeSig(crate::model::TimeSig::new(6, 8)),
        );
        first.add_segment(segment);
        score.add_measure(first);
        for index in 1..3 {
            score.add_measure(crate::model::Measure::new(
                index,
                Fraction::from_integer(index as i64),
                Fraction::new(3, 4),
            ));
        }
        let sig = timesig_at(&score, 2);
        assert_eq!((sig.numerator, sig.denominator), (6, 8));
    }

    #[test]
    fn decoupe_un_trou_sur_les_temps_forts() {
        // Trois noires de trou à partir du deuxième temps en 4/4 : MuseScore
        // coupe sur le temps fort suivant (noire, blanche).
        let sig = Sig {
            numerator: 4,
            denominator: 4,
        };
        let kinds =
            to_rhythmic_duration_list(Fraction::new(3, 4), Fraction::QUARTER, sig, Fraction::ONE);
        assert_eq!(kinds, vec![DurationType::Quarter, DurationType::Half]);
    }

    #[test]
    fn un_trou_de_mesure_entiere_reste_une_mesure() {
        let sig = Sig {
            numerator: 2,
            denominator: 4,
        };
        let kinds = to_rhythmic_duration_list(Fraction::HALF, Fraction::ZERO, sig, Fraction::HALF);
        assert_eq!(kinds, vec![DurationType::Measure]);
    }

    /// Douce nuit — chant piano, mesure 45 (6/8) : la référence MuseScore
    /// découpe le trou de 5/8 à partir du deuxième huitième en
    /// `1/8, 1/8, 1/4, 1/8` (les `<forward> 12,12,24,12` de l'export).
    #[test]
    fn trou_croche_en_mesure_6_8() {
        let sig = Sig {
            numerator: 6,
            denominator: 8,
        };
        let kinds = to_rhythmic_duration_list(
            Fraction::new(5, 8),
            Fraction::new(1, 8),
            sig,
            Fraction::new(3, 4),
        );
        eprintln!("SPLIT {:?}", kinds);
        assert_eq!(
            kinds,
            vec![
                DurationType::Eighth,
                DurationType::Eighth,
                DurationType::Quarter,
                DurationType::Eighth,
            ]
        );
    }

    #[test]
    fn une_croche_de_trou_tient_en_une_figure() {
        let sig = Sig {
            numerator: 4,
            denominator: 4,
        };
        let kinds =
            to_rhythmic_duration_list(Fraction::new(1, 8), Fraction::ZERO, sig, Fraction::ONE);
        assert_eq!(kinds, vec![DurationType::Eighth]);
    }
}
