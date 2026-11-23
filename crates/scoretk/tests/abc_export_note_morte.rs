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

//! Note morte (`dead=1` de MuseScore, `<notehead>x</notehead>`) dans un accord.
//!
//! Réduit de `voiceswitching`, mesure 3 : un accord porte une note morte F4
//! (tête en croix) et une note ordinaire C5. `xml2abc.py::doNote` (lignes
//! 1226-1229) n'écarte que les notes `print-object="no"` : la tête en croix
//! garde sa hauteur et la référence MusicXML compte les deux notes. La norme
//! ABC 2.1 §4.17 écrit un accord `[FC]4`.

use scoretk_core::model::*;

fn note_count(score: &Score) -> usize {
    score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .filter_map(|segment| segment.chord_rest(Track::new(score.staves[0].id, 0)))
        .filter_map(|chord_rest| chord_rest.chord_ref())
        .map(|chord| chord.notes.len())
        .sum()
}

#[test]
fn note_morte_dans_un_accord_ecrite_et_relue() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut dead = Note::new(65, scoretk_core::model::tpc::F);
    dead.ghost = true;
    let dead = score.add_note(dead);
    let plain = score.add_note(Note::new(72, scoretk_core::model::tpc::C));
    let mut cr = ChordRest::chord(Duration::whole());
    cr.chord_mut().unwrap().notes.extend([dead, plain]);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(cr));
    m.add_segment(seg);
    score.add_measure(m);

    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    assert!(abc.contains("[Fc]8"), "accord incomplet :\n{abc}");
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    assert_eq!(note_count(&back), 2, "note morte perdue :\n{abc}");
}
