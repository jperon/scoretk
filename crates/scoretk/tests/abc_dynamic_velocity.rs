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

//! Les nuances ABC sont des symboles (`!p!`) : leur vélocité MIDI n'a pas de
//! notation standard. La décoration privée `!scoretk-velo-N!` (ABC 2.1 §11)
//! la transporte sans changer le sens des fichiers pour les autres outils.

use scoretk::model::{
    Annotation, Dynamic, Fraction, Measure, Part, Score, Segment, SegmentType, Track,
};
use scoretk::{read, write, Format, Options};

fn partition_avec_nuance() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno"));
    let staff = score.add_staff(part);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let note = score.add_note(scoretk::model::Note::new(60, 14));
    let mut chord_rest = scoretk::model::ChordRest::chord(scoretk::model::Duration::whole());
    if let scoretk::model::ChordRestKind::Chord(chord) = &mut chord_rest.kind {
        chord.notes.push(note);
    }
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    segment.set_element(
        Track::new(staff, 0),
        scoretk::model::SegmentItem::ChordRest(chord_rest),
    );
    let mut dynamic = Dynamic::new("p");
    dynamic.velocity = Some(72);
    segment
        .annotations
        .push(scoretk::model::SegmentAnnotation::new(
            Track::new(staff, 0),
            Annotation::Dynamic(dynamic),
        ));
    measure.add_segment(segment);
    score.add_measure(measure);
    score
}

#[test]
fn velocite_de_nuance_vont_et_revient_par_abc() {
    let source = partition_avec_nuance();
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(
        text.contains("!scoretk-velo-72!!p!"),
        "décoration de vélocité : {text}"
    );
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    let velocities: Vec<Option<u8>> = back.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.annotations.iter())
        .filter_map(|entry| match &entry.annotation {
            Annotation::Dynamic(dynamic) => Some(dynamic.velocity),
            _ => None,
        })
        .collect();
    assert_eq!(velocities, [Some(72)]);
}
