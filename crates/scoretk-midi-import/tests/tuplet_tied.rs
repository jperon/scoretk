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

//! Détection d'un triolet dont la dernière note est liée sur plusieurs temps.

mod common;

use midly::num::u15;
use midly::{Format, Timing};
use scoretk_midi_import::read_midi;

use common::{smf_with, Ev};

/// Réduit de `tuplet_tied_3_5_tuplets.mid` (amont) : trois notes à 0, 1/3 et
/// 2/3 de noire, la dernière tenue jusqu'à 2,19 noires (liaison).
#[test]
fn triolet_avec_note_liee() {
    let events = vec![
        (0, Ev::TimeSig(4, 4)),
        (0, Ev::NoteOn(0, 67, 80)),
        (126, Ev::NoteOff(0, 67)),
        (128, Ev::NoteOn(0, 71, 80)),
        (252, Ev::NoteOff(0, 71)),
        (256, Ev::NoteOn(0, 72, 80)),
        (840, Ev::NoteOff(0, 72)),
    ];
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(384)), &[events]);
    let score = read_midi(&bytes).expect("lecture");
    assert!(
        !score.tuplets.is_empty(),
        "le triolet (0, 1/3, 2/3) doit être détecté malgré la note liée"
    );
}
