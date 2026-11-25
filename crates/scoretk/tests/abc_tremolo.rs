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

//! ABC 2.1 §4.14 porte `!/!` (trémolo d'une note) ; les autres genres du
//! modèle passent par une décoration privée (§11, ignorée par les autres
//! programmes) afin que l'aller-retour ne les perde pas.

use scoretk::model::{
    ChordRest, ChordRestKind, Duration, Fraction, Measure, Note, Part, Score, Segment, SegmentItem,
    SegmentType, Track, Tremolo, TremoloKind,
};
use scoretk::{read, write, Format, Options};

fn partition(kind: TremoloKind) -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno"));
    let staff = score.add_staff(part);
    let note = score.add_note(Note::new(60, 14));
    let mut rest = ChordRest::chord(Duration::whole());
    if let ChordRestKind::Chord(chord) = &mut rest.kind {
        chord.notes.push(note);
        chord.tremolo = Some(Tremolo { kind, count: 3 });
    }
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(rest));
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    measure.add_segment(segment);
    score.add_measure(measure);
    score
}

fn lu(score: &Score) -> Option<Tremolo> {
    let abc = write(score, Format::Abc, &Options::default()).expect("écriture ABC");
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    for measure in &back.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(ChordRest {
                    kind: ChordRestKind::Chord(chord),
                    ..
                }) = item
                {
                    return chord.tremolo;
                }
            }
        }
    }
    None
}

#[test]
fn tremolo_simple_vient_des_barres_abc() {
    let source = partition(TremoloKind::Single);
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc).expect("UTF-8");
    assert!(text.contains("!///!"), "{text}");
    assert_eq!(
        lu(&source),
        Some(Tremolo {
            kind: TremoloKind::Single,
            count: 3
        })
    );
}

#[test]
fn tremolo_a_deux_notes_passe_par_la_decoration_privee() {
    let source = partition(TremoloKind::Start);
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc).expect("UTF-8");
    assert!(text.contains("!scoretk-tremolo-start!"), "{text}");
    // Le genre est porté ; le nombre de barres reste dans la notation
    // standard et n'est pas reconstruit par la forme privée.
    assert_eq!(
        lu(&source),
        Some(Tremolo {
            kind: TremoloKind::Start,
            count: 0
        })
    );
}
