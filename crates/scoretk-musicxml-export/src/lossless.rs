// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! Copie « modèle complet » d'une partition pour l'export additif.
//!
//! MuseScore 4.7.4 n'écrit pas les éléments invisibles dans son MusicXML
//! (`canWrite`, `exportmusicxml.cpp:9408-9411`). C'est fidèle à la sortie de
//! référence, mais le modèle pivot, lui, les conserve. En mode
//! `ExportOptions::lossless_round_trip`, on rend ces éléments visibles sur une
//! copie avant l'export, avec des éléments MusicXML ordinaires (`<direction>`,
//! `<wedge>`, `<octave-shift>`, `<pedal>`…) : la relecture retrouve ainsi le
//! modèle complet. Les octets par défaut ne changent pas.

use scoretk_core::model::{Annotation, Measure, RepeatMark, Score, SegmentItem, SegmentType};

/// Copie de `score` où tous les objets que MuseScore masquerait à l'export
/// sont marqués visibles (mode secondaire).
pub(crate) fn make_all_visible(score: &Score) -> Score {
    make_visible(score, true)
}

/// Copie pour `full_support` : seules les directions et les spanners que
/// `canWrite` masque sont rendus visibles, afin que des porteurs XSD standard
/// (`xsd:dynamics`, `xsd:words`, `xsd:metronome`, `xsd:wedge`…) les écrivent.
/// Les notes restent invisibles : `xsd:note@print-object` porte leur état.
pub(crate) fn make_export_visible(score: &Score) -> Score {
    make_visible(score, false)
}

fn make_visible(score: &Score, notes: bool) -> Score {
    let mut copy = score.clone();
    let tracks = copy.staves.len() * scoretk_core::model::VOICES;
    for measure in &mut copy.measures {
        make_measure_visible(measure, tracks, notes);
    }
    for spanner in &mut copy.spanners {
        spanner.visible = true;
    }
    if notes {
        for note in &mut copy.notes {
            note.visible = true;
        }
    }
    copy
}

fn make_measure_visible(measure: &mut Measure, tracks: usize, notes: bool) {
    for mark in &mut measure.repeat_marks {
        match mark {
            RepeatMark::Marker(marker) => marker.visible = true,
            RepeatMark::Jump(jump) => jump.visible = true,
        }
    }
    for segment in &mut measure.segments {
        for entry in &mut segment.annotations {
            match &mut entry.annotation {
                Annotation::Dynamic(value) => value.visible = true,
                Annotation::StaffText(value) | Annotation::Expression(value) => {
                    value.visible = true
                }
                Annotation::SystemText(value) => value.visible = true,
                Annotation::Tempo(value) => value.visible = true,
                Annotation::RehearsalMark(value) => value.visible = true,
                Annotation::InstrumentChange(value) => value.visible = true,
                Annotation::FiguredBass(value) => value.visible = true,
                Annotation::Harmony(_)
                | Annotation::Fermata(_)
                | Annotation::TremoloBar(_)
                | Annotation::Other { .. } => {}
            }
        }
        if segment.kind != SegmentType::ChordRest || !notes {
            continue;
        }
        for raw in 0..tracks as u32 {
            let track = scoretk_core::model::Track::from_raw(raw);
            if let Some(SegmentItem::ChordRest(chord_rest)) = segment.element_mut(track) {
                if let Some(rest) = chord_rest.rest_mut() {
                    rest.visible = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Annotation, Fraction, Note, StaffText, Tempo};

    #[test]
    fn copie_rend_visibles_textes_notes_et_spanners() {
        let mut score = Score::new();
        let part = score.add_part(scoretk_core::model::Part::new("P", "P"));
        let staff = score.add_staff(part);
        let mut measure = scoretk_core::model::Measure::new(
            1,
            Fraction::ZERO,
            scoretk_core::model::Fraction::ONE,
        );
        let mut text = StaffText::new("invisible");
        text.visible = false;
        let segment = scoretk_core::model::Segment::new(
            SegmentType::ChordRest,
            Fraction::ZERO,
            score.staves.len(),
        );
        let mut segment = segment;
        segment.add_annotation(
            scoretk_core::model::Track::new(staff, 0),
            Annotation::StaffText(text),
        );
        measure.add_segment(segment);
        score.add_measure(measure);
        let mut note = Note::new(60, 14);
        note.visible = false;
        score.notes.push(note);

        let copy = make_all_visible(&score);
        assert!(score.engraving.credits.is_empty());
        assert!(!score.notes[0].visible);
        assert!(copy.notes[0].visible);
        assert!(copy.measures[0].segments[0].annotations[0]
            .annotation
            .visible());
    }

    #[test]
    fn copie_xsd_garde_les_notes_invisibles() {
        let mut score = Score::new();
        let part = score.add_part(scoretk_core::model::Part::new("P", "P"));
        let staff = score.add_staff(part);
        let mut measure = scoretk_core::model::Measure::new(
            1,
            Fraction::ZERO,
            scoretk_core::model::Fraction::ONE,
        );
        let mut text = StaffText::new("invisible");
        text.visible = false;
        let segment = scoretk_core::model::Segment::new(
            SegmentType::ChordRest,
            Fraction::ZERO,
            score.staves.len(),
        );
        let mut segment = segment;
        segment.add_annotation(
            scoretk_core::model::Track::new(staff, 0),
            Annotation::StaffText(text),
        );
        measure.add_segment(segment);
        score.add_measure(measure);
        let mut note = Note::new(60, 14);
        note.visible = false;
        score.notes.push(note);

        let copy = make_export_visible(&score);
        assert!(!copy.notes[0].visible, "xsd:note@print-object porte l'état");
        assert!(copy.measures[0].segments[0].annotations[0]
            .annotation
            .visible());
        assert!(!score.measures[0].segments[0].annotations[0]
            .annotation
            .visible());
    }

    #[test]
    fn tempo_invisible_devient_visible() {
        let mut tempo = Tempo::new("90");
        tempo.visible = false;
        assert!(!tempo.visible);
        let mut annotation = Annotation::Tempo(tempo);
        match &mut annotation {
            Annotation::Tempo(value) => value.visible = true,
            _ => unreachable!(),
        }
        assert!(annotation.visible());
    }
}
