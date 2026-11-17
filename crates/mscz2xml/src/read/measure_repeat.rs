// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/read410/measureread.cpp` de MuseScore Studio,
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

//! Mesures répétées (`<RepeatMeasure>`, `<MeasureRepeat>`).
//!
//! MuseScore en fait un `MeasureRepeat` posé sur le segment de la mesure ; son
//! contenu est celui des mesures précédentes. L'export MusicXML de MuseScore
//! repointe la mesure vers son contenu d'origine et écrit en plus
//! `<measure-style><measure-repeat>`. Le modèle, lui, recopie ce contenu dans
//! la mesure qui répète : accords et silences, annotations de segment et
//! spanners entièrement contenus dans la mesure source.
//!
//! Portage de la branche `MeasureRepeat`/`RepeatMeasure` de
//! `MeasureRead::readVoice` (`engraving/rw/read410/measureread.cpp`) et de
//! `ExportMusicXml::measureRepeat`.

use crate::model::{
    measure::MeasureRepeatMark, Annotation, ChordRest, ChordRestKind, NoteId, Score, SegmentItem,
    SegmentType, StaffId, Tick, Track,
};


/// Mesure marquée `<RepeatMeasure>` : son contenu est celui des mesures
/// précédentes.
pub(crate) struct MeasureRepeat {
    /// Index de la mesure qui répète.
    pub(crate) measure: usize,
    /// Portée concernée.
    pub(crate) staff: usize,
    /// Nombre de mesures répétées (`numMeasures`).
    pub(crate) count: u16,
}

/// Marque la mesure comme répétée et recopie le contenu de la source.
///
/// La copie porte sur la portée concernée, comme `MeasureRepeat` dans
/// MuseScore : les autres portées de la mesure restent vides.
pub(crate) fn expand(score: &mut Score, repeat: &MeasureRepeat) {
    let MeasureRepeat {
        measure,
        staff,
        count,
    } = *repeat;
    if measure == 0 || measure >= score.measures.len() {
        return;
    }
    let count = count.max(1);
    let staff_id = StaffId::from_index(staff);
    score.measures[measure].measure_repeat = Some(MeasureRepeatMark {
        staff: staff_id,
        count,
    });

    let source_index = measure.saturating_sub(usize::from(count));
    if source_index >= measure {
        return;
    }
    let source_start = score.measures[source_index].start_tick;
    let source_end = score.measures[source_index].end_tick();
    let target_start = score.measures[measure].start_tick;
    let delta: Tick = target_start - source_start;

    // Accords et silences de la portée répétée, notes recopiées.
    let mut chords: Vec<(Track, ChordRest, Tick)> = Vec::new();
    for segment in &score.measures[source_index].segments {
        for (track, item) in segment.iter_elements() {
            if track.staff() != staff_id {
                continue;
            }
            let SegmentItem::ChordRest(chord_rest) = item else {
                continue;
            };
            chords.push((track, chord_rest.clone(), segment.tick - source_start));
        }
    }
    for (track, mut chord_rest, offset) in chords {
        if let ChordRestKind::Chord(chord) = &mut chord_rest.kind {
            chord.notes = duplicate_notes(score, &chord.notes);
            for grace in &mut chord.grace_chords {
                grace.chord.notes = duplicate_notes(score, &grace.chord.notes);
            }
        }
        segment_mut(&mut score.measures[measure], target_start + offset)
            .set_element(track, SegmentItem::ChordRest(chord_rest));
    }

    // Annotations des segments d'accords : MuseScore les ré-exporte avec le
    // contenu de la mesure source. Le C++ ne les reprend que sur les segments
    // `ChordRest` (les annotations d'un segment `TimeTick` ne sont pas
    // réécrites, comme le repère `B` de `layout_elements`).
    let mut annotations: Vec<(Track, Annotation, Tick)> = Vec::new();
    for segment in &score.measures[source_index].segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        // Une annotation sans accord ni silence dans son segment est écrite en
        // fin de mesure par `annotationsWithoutNote`, à partir de la mesure
        // *répétée* : le C++ ne la reprend donc pas dans le contenu re-pointé
        // (repère `B` de `layout_elements`).
        let has_element = (0..crate::model::VOICES as u8)
            .any(|voice| segment.element(Track::new(staff_id, voice)).is_some());
        if !has_element {
            continue;
        }
        for entry in &segment.annotations {
            if entry.track.staff() != staff_id {
                continue;
            }
            annotations.push((
                entry.track,
                entry.annotation.clone(),
                segment.tick - source_start,
            ));
        }
    }
    for (track, annotation, offset) in annotations {
        segment_mut(&mut score.measures[measure], target_start + offset)
            .add_annotation(track, annotation);
    }

    // Spanners entièrement contenus dans la mesure source : glissandos,
    // liaisons d'expression, soufflets…, décalés avec le contenu.
    let mut spanners = Vec::new();
    for spanner in &score.spanners {
        if spanner.start.track.staff() != staff_id
            || spanner.start.tick < source_start
            || spanner.end.tick > source_end
        {
            continue;
        }
        let mut copy = spanner.clone();
        copy.start.tick += delta;
        copy.end.tick += delta;
        spanners.push(copy);
    }
    score.spanners.extend(spanners);
}

/// Segment de la mesure, créé s'il n'existe pas (`Measure::getSegment`).
fn segment_mut(measure: &mut crate::model::Measure, tick: Tick) -> &mut crate::model::Segment {
    if let Some(position) = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == SegmentType::ChordRest)
    {
        return &mut measure.segments[position];
    }
    measure.add_segment(crate::model::Segment::new(
        SegmentType::ChordRest,
        tick,
        1,
    ));
    let position = measure
        .segments
        .iter()
        .position(|segment| segment.tick == tick && segment.kind == SegmentType::ChordRest)
        .expect("le segment vient d'être inséré");
    &mut measure.segments[position]
}

/// Copie les notes d'un accord pour une mesure répétée.
///
/// Les liaisons de prolongation restent celles de la source : l'export ne lit
/// que leur présence, et MuseScore ré-exporte le contenu de la mesure source
/// avec ses liaisons.
fn duplicate_notes(score: &mut Score, notes: &[NoteId]) -> Vec<NoteId> {
    let mut copies = Vec::with_capacity(notes.len());
    for &note in notes {
        let copy = score.note(note).clone();
        copies.push(score.add_note(copy));
    }
    copies
}
