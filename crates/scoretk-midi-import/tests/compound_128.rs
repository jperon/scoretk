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

//! Temps pointé d'une mesure à 12 huitièmes.

mod common;

use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};
use scoretk_midi_import::read_midi;

use common::{smf_with, Ev};

fn first_duration(score: &Score) -> Option<Fraction> {
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(cr) = item {
                    if let ChordRestKind::Chord(_) = &cr.kind {
                        return cr.ticks;
                    }
                }
            }
        }
    }
    None
}

/// En 12/8, `beatLength` (`importmidi_meter.cpp:135-147`) donne un temps de
/// croche pointée (1/8 de noire) : la mesure réduite 3/4 donnerait à tort une
/// noire entière et les durées simplifiées seraient deux fois trop longues.
#[test]
fn temps_pointe_en_12_8() {
    let mut events = vec![(0, Ev::TimeSig(12, 8)), (0, Ev::Tempo(500_000))];
    for i in 0..12u32 {
        let tick = 240 * i; // croches, division = 480
        events.push((tick, Ev::NoteOn(0, 60 + (i % 5) as u8, 80)));
        events.push((tick + 360, Ev::NoteOff(0, 60 + (i % 5) as u8)));
    }
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    let duration = first_duration(&score).expect("première note");
    assert!(
        duration <= Fraction::new(1, 8),
        "la première durée doit tenir dans une croche pointée, obtenu {duration:?}"
    );
}
