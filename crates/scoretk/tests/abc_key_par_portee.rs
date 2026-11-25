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

//! ABC 2.1 §3.2/§7.3 : un champ `[K:…]` en ligne est local à la voix où il
//! apparaît. Deux portées d'une même partie peuvent donc porter des armures
//! différentes ; l'en-tête `K:` global ne suffit pas.

use scoretk::model::{
    Fraction, Key, KeySig, Measure, Part, Score, Segment, SegmentItem, SegmentType, Track,
};
use scoretk::{read, write, Format, Options};

fn partition_deux_armures() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno"));
    let first = score.add_staff(part);
    let second = score.add_staff(part);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    for (staff, key) in [(first, Key::C), (second, Key::G)] {
        let mut segment = Segment::new(SegmentType::KeySig, Fraction::ZERO, 2);
        segment.set_element(Track::new(staff, 0), SegmentItem::KeySig(KeySig::new(key)));
        measure.add_segment(segment);
    }
    score.add_measure(measure);
    score
}

#[test]
fn armure_locale_de_la_seconde_portee_survit_a_l_aller_retour() {
    let source = partition_deux_armures();
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(
        text.contains("[K:G"),
        "armure de la seconde portée : {text}"
    );
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    let track = Track::new(back.staves[1].id, 0);
    let keys: Vec<KeySig> = back.measures[0]
        .segments
        .iter()
        .filter(|segment| segment.kind == SegmentType::KeySig)
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter(move |(item_track, _)| *item_track == track)
                .filter_map(|(_, item)| match item {
                    SegmentItem::KeySig(key) => Some(key.clone()),
                    _ => None,
                })
        })
        .collect();
    assert!(
        keys.iter().any(|key| key.concert_key.fifths() == 1),
        "la seconde portée doit retrouver sol majeur : {keys:?}"
    );
}
