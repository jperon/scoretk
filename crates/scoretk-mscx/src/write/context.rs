// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/writecontext.h` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! État partagé pendant l'écriture.
//!
//! Le modèle ne porte pas de pointeur de parent : les positions des notes
//! (pour les liaisons), les moitiés de spanners et l'étirement de portée en
//! vigueur sont précalculés ici, comme `WriteContext` les mémorise au fil de
//! l'écriture dans MuseScore.

use std::collections::BTreeMap;

use mscz2xml::model::{
    ChordRestKind, Fraction, Measure, NoteId, Score, SegmentItem, SegmentType, Spanner,
    SpannerType, Tick, Track,
};

/// Moitié de spanner à écrire, avec la position de l'autre extrémité.
#[derive(Debug)]
pub(crate) struct Half<'a> {
    /// Rang du spanner dans l'arène, pour un ordre de sortie stable.
    pub order: usize,
    /// Spanner d'origine.
    pub spanner: &'a Spanner,
    /// Vrai pour la moitié de départ (`<next>`).
    pub start: bool,
    /// Tick de l'autre extrémité.
    pub other_tick: Tick,
    /// Piste de l'autre extrémité.
    pub other_track: Track,
}

/// Marque de mesure répétée : portée et nombre de mesures.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RepeatTarget {
    /// Portée dont le contenu est recopié.
    pub staff: usize,
    /// Nombre de mesures répétées.
    pub count: u16,
}

/// État d'écriture d'une partition.
pub(crate) struct WriteContext<'a> {
    /// Partition écrite.
    pub score: &'a Score,
    /// Position de chaque note : tick du segment et piste.
    note_positions: BTreeMap<NoteId, (Tick, Track)>,
    /// Moitiés de spanners, rangées par (piste brute, tick).
    halves: BTreeMap<(u32, Fraction), Vec<Half<'a>>>,
    /// Mesures répétées, indexées par mesure.
    repeats: Vec<Option<RepeatTarget>>,
    /// Étirement de portée en vigueur au début de chaque mesure, par portée.
    stretch: Vec<Vec<Fraction>>,
}

impl<'a> WriteContext<'a> {
    /// Prépare l'écriture d'une partition.
    pub fn new(score: &'a Score) -> Self {
        let mut context = Self {
            score,
            note_positions: BTreeMap::new(),
            halves: BTreeMap::new(),
            repeats: vec![None; score.measures.len()],
            stretch: Vec::new(),
        };
        context.index_notes();
        context.index_repeats();
        context.index_halves();
        context.index_stretch();
        context
    }

    /// Position d'une note, pour les liaisons de prolongation.
    pub fn note_position(&self, note: NoteId) -> Option<(Tick, Track)> {
        self.note_positions.get(&note).copied()
    }

    /// Moitiés de spanners d'une piste dont la position tombe dans une mesure.
    ///
    /// Chaque moitié appartient à la mesure qui contient son tick
    /// (`Measure::measureIndex`) : une moitié posée sur une barre de mesure est
    /// donc écrite une seule fois, dans la mesure suivante.
    pub fn halves_for(&self, measure_index: usize, track: Track) -> Vec<&Half<'a>> {
        let measures = &self.score.measures;
        self.halves
            .iter()
            .filter(|((raw, _), _)| *raw == track.raw())
            .flat_map(|(_, halves)| halves.iter())
            .filter(|half| {
                let tick = if half.start {
                    half.spanner.start.tick
                } else {
                    half.spanner.end.tick
                };
                measure_index_at(measures, tick) == Some(measure_index)
            })
            .collect()
    }

    /// Marque de mesure répétée, si la mesure en porte une.
    pub fn repeat_target(&self, measure: usize) -> Option<RepeatTarget> {
        self.repeats.get(measure).copied().flatten()
    }

    /// Étirement de portée au début d'une mesure.
    pub fn stretch_at(&self, measure: usize, staff: usize) -> Fraction {
        self.stretch
            .get(measure)
            .and_then(|per_staff| per_staff.get(staff))
            .copied()
            .unwrap_or(Fraction::ONE)
    }

    /// Position d'une note dans le contenu de la portée, pour les liaisons.
    fn index_notes(&mut self) {
        for measure in &self.score.measures {
            for segment in &measure.segments {
                for (track, item) in segment.iter_elements() {
                    let SegmentItem::ChordRest(chord_rest) = item else {
                        continue;
                    };
                    let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                        continue;
                    };
                    for note in &chord.notes {
                        self.note_positions
                            .entry(*note)
                            .or_insert((segment.tick, track));
                    }
                    for grace in &chord.grace_chords {
                        for note in &grace.chord.notes {
                            self.note_positions
                                .entry(*note)
                                .or_insert((segment.tick, track));
                        }
                    }
                }
            }
        }
    }

    /// Relève les mesures répétées (`<MeasureRepeat>`).
    fn index_repeats(&mut self) {
        for (index, measure) in self.score.measures.iter().enumerate() {
            if let Some(mark) = measure.measure_repeat {
                self.repeats[index] = Some(RepeatTarget {
                    staff: mark.staff.index(),
                    count: mark.count.max(1),
                });
            }
        }
    }

    /// Répartit les moitiés de spanners par position.
    ///
    /// Les spanners recopiés dans une mesure répétée ne sont pas écrits : le
    /// lecteur les recrée en développant la mesure source.
    fn index_halves(&mut self) {
        for (order, spanner) in self.score.spanners.iter().enumerate() {
            if matches!(spanner.kind, SpannerType::Other(_)) {
                continue;
            }
            if self.is_repeat_copy(spanner) {
                continue;
            }
            self.halves
                .entry((spanner.start.track.raw(), spanner.start.tick))
                .or_default()
                .push(Half {
                    order,
                    spanner,
                    start: true,
                    other_tick: spanner.end.tick,
                    other_track: spanner.end.track,
                });
            self.halves
                .entry((spanner.end.track.raw(), spanner.end.tick))
                .or_default()
                .push(Half {
                    order,
                    spanner,
                    start: false,
                    other_tick: spanner.start.tick,
                    other_track: spanner.start.track,
                });
        }
        // L'ordre des spanners de même type est significatif pour la parité :
        // il suit l'arène du modèle.
        for halves in self.halves.values_mut() {
            halves.sort_by_key(|half| (half.order, !half.start));
        }
    }

    /// Vrai si un spanner est une copie produite par une mesure répétée.
    fn is_repeat_copy(&self, spanner: &Spanner) -> bool {
        let staff = spanner.start.track.staff().index();
        let measure_index = match measure_index_at(&self.score.measures, spanner.start.tick) {
            Some(index) => index,
            None => return false,
        };
        let Some(target) = self.repeat_target(measure_index) else {
            return false;
        };
        if target.staff != staff {
            return false;
        }
        let measure = &self.score.measures[measure_index];
        spanner.start.tick >= measure.start_tick && spanner.end.tick <= measure.end_tick()
    }

    /// Reconstruit l'étirement de portée mesure par mesure.
    fn index_stretch(&mut self) {
        let staff_count = self.score.staves.len();
        let mut current = vec![Fraction::ONE; staff_count];
        for measure in &self.score.measures {
            for segment in &measure.segments {
                if segment.kind != SegmentType::TimeSig || segment.tick != measure.start_tick {
                    continue;
                }
                let Some(SegmentItem::TimeSig(time_sig)) = segment
                    .iter_elements()
                    .map(|(_, item)| item)
                    .find(|item| matches!(item, SegmentItem::TimeSig(_)))
                else {
                    continue;
                };
                if !time_sig.stretch.is_positive() {
                    continue;
                }
                for (track, _) in segment.iter_elements() {
                    let staff = track.staff().index();
                    if staff < staff_count {
                        current[staff] = time_sig.stretch;
                    }
                }
            }
            self.stretch.push(current.clone());
        }
    }
}

/// Index de la mesure contenant un tick (`Measure::measureIndex`).
pub(crate) fn measure_index_at(measures: &[Measure], tick: Tick) -> Option<usize> {
    let index = measures.partition_point(|measure| measure.start_tick <= tick);
    index.checked_sub(1).filter(|index| *index < measures.len())
}
