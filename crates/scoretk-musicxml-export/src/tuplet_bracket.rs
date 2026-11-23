// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `Tuplet::calcHasBracket` (`src/engraving/dom/tuplet.cpp:204-277`),
// de `Beam::calcBeamBreaks` (`src/engraving/dom/beam.cpp:248-297`) et de
// `TupletLayout::computeDirection` (4.7.4
// `src/engraving/rendering/score/tupletlayout.cpp:213-245`) de MuseScore
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

//! Décision de crochet des n-olets quand `<bracketType>` est `AUTO_BRACKET`.
//!
//! `Tuplet::calcHasBracket` ne retire le crochet que si le n-olet possède son
//! propre objet `Beam` (ou si les ruptures de ligature délimitent le n-olet),
//! et seulement quand `!headSide || isCross`. On porte ici la chaîne complète
//! des prédicats (`tuplet.cpp:218-277`, `beam.cpp:248-297`) sur les objets
//! `Beam` reconstitués par `beams.rs`.

use scoretk_core::model::{
    Chord, ChordRest, Fraction, Placement, Score, SegmentItem, SegmentType, StaffId, StemDirection,
    Tick, Track, TupletId,
};

use super::{beams, stems};

/// Élément rythmique et sa position, pour les prédicats de ligature.
struct Elem<'a> {
    measure: usize,
    track: Track,
    tick: Fraction,
    rest: &'a ChordRest,
    visible: bool,
}

impl Elem<'_> {
    fn cr_ref(&self) -> beams::CrRef {
        beams::CrRef {
            measure: self.measure,
            track: self.track,
            tick: self.tick,
        }
    }

    fn beam_cr(&self) -> beams::BeamCr<'_> {
        beams::BeamCr {
            measure: self.measure,
            track: self.track,
            tick: self.tick,
            rest: self.rest,
        }
    }
}

/// Vrai si MuseScore dessinerait un crochet autour du n-olet.
pub(crate) fn has_bracket(score: &Score, id: TupletId, concert_pitch: bool) -> bool {
    let mut all = Vec::new();
    for (measure_index, measure) in score.measures.iter().enumerate() {
        for segment in &measure.segments {
            if segment.kind != SegmentType::ChordRest {
                continue;
            }
            for (track, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(chord_rest) = item {
                    all.push(Elem {
                        measure: measure_index,
                        track,
                        tick: segment.tick,
                        rest: chord_rest,
                        visible: score.staff(track.staff()).visible,
                    });
                }
            }
        }
    }
    // `Tuplet::elements()` contient aussi les n-olets imbriqués ;
    // `TupletLayout::computeStartEndCR` (4.7.4 `tupletlayout.cpp:244-265`)
    // descend jusqu'au premier et dernier accord réel, qui peuvent appartenir
    // à un n-olet fils et donc à une ligature différente.
    let elements: Vec<&Elem<'_>> = all
        .iter()
        .filter(|element| {
            element
                .rest
                .tuplet
                .is_some_and(|member| belongs_to(score, member, id))
        })
        .collect();

    // `cr1 == cr2` (n-olet dégénéré) : pas de crochet (`tuplet.cpp:209-211`).
    if elements.len() < 2 {
        return false;
    }
    // Une portée masquée n'est pas disposée : `hasBracket` reste à sa valeur
    // par défaut (faux), même pour un triolet avec silence. C'est ce qui
    // sépare les 31 crochets de Douce nuit (portée visible Femmes) des 31
    // triolets identiques sur portées masquées.
    if elements.iter().all(|element| !element.visible) {
        return false;
    }

    let first = elements[0];
    let last = *elements.last().expect("au moins deux éléments");
    // `!cr1->isChord() || !cr2->isChord()` (`tuplet.cpp:212-214`).
    if first.rest.is_rest() || last.rest.is_rest() {
        return true;
    }

    let beams = beams::Beams::new(score, concert_pitch);
    let beam_start = beams.group_of(first.measure, first.track, first.tick);
    let beam_end = beams.group_of(last.measure, last.track, last.tick);
    // `!beamStart || !beamEnd || beamStart != beamEnd` (`tuplet.cpp:218-222`).
    let (Some(beam_start), Some(beam_end)) = (beam_start, beam_end) else {
        return true;
    };
    if beam_start.start != beam_end.start || beam_start.end != beam_end.end {
        return true;
    }

    // `beamStart->elements().front() == c1`, `.back() == c2`
    // (`tuplet.cpp:223-224`) : le n-olet possède-t-il toute la ligature ?
    let tuplet_starts_beam = beam_start.start == first.cr_ref();
    let tuplet_ends_beam = beam_start.end == last.cr_ref();

    let is_up = tuplet_direction_up(score, &elements, concert_pitch, &beams);
    let first_up = chord_up(score, first, concert_pitch, &beams);
    let last_up = chord_up(score, last, concert_pitch, &beams);
    // `headSide = isUp() != (c1->up() || c2->up())` (`tuplet.cpp:225`).
    let head_side = is_up != (first_up || last_up);
    // `isCross = c1->vStaffIdx() != c2->vStaffIdx()` (`tuplet.cpp:226`).
    let cross = vstaff(first.track, first.rest) != vstaff(last.track, last.rest);
    if tuplet_starts_beam && tuplet_ends_beam && (!head_side || cross) {
        return false;
    }

    // `for (DurationElement* e : m_currentElements)` (`tuplet.cpp:231-249`) :
    // silence, n-olet imbriqué, élément sans beam ou nombre de crochets
    // différent → crochet conservé.
    let beam_count = beams::stems_hooks(first.rest.duration.kind);
    for element in &elements {
        if element.rest.tuplet != Some(id)
            || element.rest.is_rest()
            || beams
                .group_of(element.measure, element.track, element.tick)
                .is_none()
            || beams::stems_hooks(element.rest.duration.kind) != beam_count
        {
            return true;
        }
    }
    if beam_count < 1 {
        return true;
    }

    let level = i32::from(beam_count) - 1;
    let previous = previous_cr(&all, first);
    let next = next_cr(&all, last);
    let start_breaks = previous.is_some_and(|previous| {
        beams::calc_beam_breaks(score, &first.beam_cr(), &previous.beam_cr(), level)
            != (false, false)
    });
    let end_breaks = next.is_some_and(|next| {
        beams::calc_beam_breaks(score, &next.beam_cr(), &last.beam_cr(), level) != (false, false)
    });
    let start_defines = start_breaks
        || tuplet_starts_beam
        || previous
            .is_some_and(|previous| beams::stems_hooks(previous.rest.duration.kind) < beam_count);
    let end_defines = end_breaks
        || tuplet_ends_beam
        || next.is_some_and(|next| beams::stems_hooks(next.rest.duration.kind) < beam_count);

    // `tuplet.cpp:273-275`.
    !(start_defines && end_defines && (!head_side || cross))
}

/// Vrai si `member` est `id` ou descend de `id` (`Tuplet::tuplet()`).
fn belongs_to(score: &Score, member: TupletId, id: TupletId) -> bool {
    let mut current = Some(member);
    for _ in 0..32 {
        match current {
            Some(candidate) if candidate == id => return true,
            Some(candidate) => {
                current = score
                    .tuplets
                    .get(candidate.index())
                    .and_then(|tuplet| tuplet.parent);
            }
            None => return false,
        }
    }
    false
}

/// `Chord::prev()` : élément rythmique précédent de la même piste.
fn previous_cr<'a>(all: &'a [Elem<'a>], element: &Elem<'_>) -> Option<&'a Elem<'a>> {
    all.iter()
        .rposition(|candidate| same_position(candidate, element))
        .and_then(|position| {
            all[..position]
                .iter()
                .rev()
                .find(|candidate| candidate.track == element.track)
        })
}

/// `Chord::next()` : élément rythmique suivant de la même piste.
fn next_cr<'a>(all: &'a [Elem<'a>], element: &Elem<'_>) -> Option<&'a Elem<'a>> {
    all.iter()
        .skip_while(|candidate| !same_position(candidate, element))
        .skip(1)
        .find(|candidate| candidate.track == element.track)
}

fn same_position(left: &Elem<'_>, right: &Elem<'_>) -> bool {
    left.cr_ref() == right.cr_ref()
}

/// `TupletLayout::computeDirection` (`tupletlayout.cpp:213-245`, 4.7.4).
fn tuplet_direction_up(
    score: &Score,
    elements: &[&Elem<'_>],
    concert_pitch: bool,
    beams: &beams::Beams,
) -> bool {
    if let Some(placement) = elements[0]
        .rest
        .tuplet
        .and_then(|id| score.tuplets.get(id.index()))
    {
        if let Some(direction) = placement.placement {
            return direction == Placement::Above;
        }
    }
    let mut up = 0;
    for element in elements {
        let Some(chord) = element.rest.chord_ref() else {
            continue;
        };
        up += if chord.stem != StemDirection::Auto {
            if chord.stem == StemDirection::Up {
                1000
            } else {
                -1000
            }
        } else if chord_up(score, element, concert_pitch, beams) {
            1
        } else {
            -1
        };
    }
    if up == 0 {
        let measure = &score.measures[elements[0].measure];
        let staff: StaffId = elements[0].track.staff();
        let has_voices = measure
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentType::ChordRest)
            .any(|segment| {
                segment.element(Track::new(staff, 1)).is_some()
                    || segment.element(Track::new(staff, 2)).is_some()
                    || segment.element(Track::new(staff, 3)).is_some()
            });
        if has_voices {
            return elements[0].track.voice().is_multiple_of(2);
        }
        return true;
    }
    up > 0
}

/// `Chord::up()` effectif, calculé comme à l'écriture de la hampe.
fn chord_up(score: &Score, element: &Elem<'_>, concert_pitch: bool, beams: &beams::Beams) -> bool {
    let Some(chord) = element.rest.chord_ref() else {
        return true;
    };
    stem_up(
        score,
        chord,
        element.measure,
        element.track,
        element.tick,
        element.rest.duration.ticks(),
        concert_pitch,
        beams,
    )
}

#[allow(clippy::too_many_arguments)]
fn stem_up(
    score: &Score,
    chord: &Chord,
    measure_index: usize,
    track: Track,
    tick: Tick,
    actual_ticks: Fraction,
    concert_pitch: bool,
    beams: &beams::Beams,
) -> bool {
    let measure = &score.measures[measure_index];
    let staff = score.staff(track.staff());
    let part = score.part(staff.part);
    let transpose = part
        .instrument
        .transpose
        .as_ref()
        .map_or(0, |transpose| transpose.chromatic);
    stems::stem_up(
        score,
        chord,
        measure,
        track,
        tick,
        actual_ticks,
        concert_pitch,
        transpose,
        beams.up(measure_index, track, tick),
    )
}

/// `Chord::vStaffIdx()` : portée réelle après `staffMove`.
fn vstaff(track: Track, element: &ChordRest) -> i32 {
    let staff_move = element
        .chord_ref()
        .map_or(0, |chord| i32::from(chord.staff_move));
    track.staff().raw() as i32 + staff_move
}
