// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : reconstruction des barres de fin de mesure que
// le modèle de MuseScore conserve sur le segment `EndBarLine`.
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

//! Barres de fin de mesure implicites.
//!
//! MEI porte la barre finale dans `@right` de `<measure>`
//! (`format-ref/mei.md` §5, `meiconverter.cpp:574-625`), pas dans un objet
//! séparé. MuseScore conserve pourtant un `BarLine` sur le segment
//! `EndBarLine` de chaque mesure, même quand son style est `Normal`
//! (`engraving/dom/barline.h`). Le lecteur MusicXML fait déjà cette
//! reconstruction (`scoretk-musicxml-import/src/pass2.rs`); la même passe est
//! appliquée ici après la lecture des mesures afin que l'inventaire Compare
//! les mêmes objets. Une mesure à reprise finale n'a pas de tel segment dans
//! le modèle de référence : le marqueur de reprise le remplace.

use scoretk_core::model::{BarLine, Score, SegmentItem, SegmentType, StaffId, Track};

use crate::reader::Reader;

/// Recrée les segments `EndBarLine` à partir de `Measure::end_bar_line`.
pub(crate) fn finalize(score: &mut Score) {
    let staff_count = score.staves.len().max(1);
    for measure in &mut score.measures {
        let kind = measure.end_bar_line;
        if kind.is_repeat_end() {
            continue;
        }
        let tick = measure.end_tick();
        for staff_idx in 0..staff_count {
            let track = Track::new(StaffId::from_index(staff_idx), 0);
            if let Some(segment) = measure
                .segments
                .iter_mut()
                .find(|segment| segment.tick == tick && segment.kind == SegmentType::EndBarLine)
            {
                if segment.element(track).is_none() {
                    segment.set_element(track, SegmentItem::BarLine(BarLine::new(kind)));
                }
            } else {
                Reader::insert_segment(
                    measure,
                    staff_count,
                    tick,
                    SegmentType::EndBarLine,
                    staff_idx,
                    SegmentItem::BarLine(BarLine::new(kind)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarLineType, Fraction, Measure, Part};

    fn score_with_measure(kind: BarLineType) -> Score {
        let mut score = Score::new();
        let part = score.add_part(Part::new("Piano", "Pno"));
        score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::new(4, 4));
        measure.end_bar_line = kind;
        score.add_measure(measure);
        score
    }

    fn barline_at_end(score: &Score) -> Option<BarLineType> {
        let measure = &score.measures[0];
        let tick = measure.end_tick();
        measure
            .segments_of_type(SegmentType::EndBarLine)
            .find(|segment| segment.tick == tick)
            .and_then(|segment| segment.element(Track::new(StaffId::from_index(0), 0)))
            .and_then(|item| match item {
                SegmentItem::BarLine(bar) => Some(bar.kind),
                _ => None,
            })
    }

    #[test]
    fn mei_barline_renditions_become_end_segments() {
        for (rendition, expected) in [
            (BarLineType::Normal, BarLineType::Normal),
            (BarLineType::Double, BarLineType::Double),
            (BarLineType::End, BarLineType::End),
        ] {
            let mut score = score_with_measure(rendition);
            finalize(&mut score);
            assert_eq!(barline_at_end(&score), Some(expected));
        }
    }

    #[test]
    fn repeat_end_has_no_barline_segment() {
        let mut score = score_with_measure(BarLineType::EndRepeat);
        finalize(&mut score);
        assert_eq!(barline_at_end(&score), None);
    }
}
