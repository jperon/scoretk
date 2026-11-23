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

//! Un silence dont `ticks` dépasse la fin de la barre doit être tronqué à la
//! longueur de cette barre (ABC 2.1 §4.3/§4.5 ; `xml2abc.py::sortMeasure`
//! lignes 714-760 remplace le trou par un `x` de la durée restante). Réduit
//! de `Perosi`, voix 3 mesure 8 : un silence invisible d'une ronde (`x8`)
//! dans une mesure 3/4 (6 unités) allongeait la barre à 8 unités et décalait
//! toutes les attaques suivantes à la relecture.

use scoretk_core::model::*;

#[test]
fn silence_tronque_a_la_fin_de_barre() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Voix", "Vx"));
    let staff = score.add_staff(part);
    let track = Track::new(score.staff(staff).id, 0);
    let mut m = Measure::new(0, Fraction::ZERO, Fraction::new(3, 4));
    let mut rest = ChordRest::rest(Duration::whole());
    if let ChordRestKind::Rest(r) = &mut rest.kind {
        r.visible = false;
    }
    rest.ticks = Some(Fraction::ONE); // ronde entière dans une mesure 3/4
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    seg.set_element(track, SegmentItem::ChordRest(rest));
    m.add_segment(seg);
    score.add_measure(m);

    let mut m = Measure::new(1, Fraction::new(3, 4), Fraction::from_integer(2));
    let note = score.add_note(Note::new(72, scoretk_core::model::tpc::C));
    let mut cr = ChordRest::chord(Duration::whole());
    cr.chord_mut().unwrap().notes.push(note);
    let mut seg = Segment::new(SegmentType::ChordRest, Fraction::new(3, 4), 1);
    seg.set_element(track, SegmentItem::ChordRest(cr));
    m.add_segment(seg);
    score.add_measure(m);

    let abc = String::from_utf8(
        scoretk::write(&score, scoretk::Format::Abc, &scoretk::Options::default()).unwrap(),
    )
    .unwrap();
    assert!(abc.contains("x6 |"), "silence non tronqué :\n{abc}");
    let back = scoretk::read(abc.as_bytes(), scoretk::Format::Abc).unwrap();
    assert_eq!(
        back.measures[0].len,
        Fraction::new(3, 4),
        "barre allongée :\n{abc}"
    );
}
