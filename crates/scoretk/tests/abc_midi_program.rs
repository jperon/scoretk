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

//! ABC §6.2 / §11.2 : `%%MIDI program` et `%%MIDI channel` transportent le
//! programme GM et le canal, deux éléments L4 du modèle.

use scoretk::model::{Fraction, Measure, Part, Score};
use scoretk::{read, write, Format, Options};

fn partition_avec_programme() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno"));
    score.parts[part.index()].instrument.midi_program = Some(0);
    score.parts[part.index()].instrument.midi_channel = Some(2);
    score.add_staff(part);
    score.add_measure(Measure::new(0, Fraction::ZERO, Fraction::ONE));
    score
}

#[test]
fn programme_et_canal_vont_et_reviennent_par_abc() {
    let source = partition_avec_programme();
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let texte = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(
        texte.contains("%%MIDI program 1\n"),
        "GM 0 (wire) s'écrit 1 dans ABC : {texte}"
    );
    assert!(texte.contains("%%MIDI channel 3\n"));
    let relu = read(&abc, Format::Abc).expect("lecture ABC");
    assert_eq!(relu.parts[0].instrument.midi_program, Some(0));
    assert_eq!(relu.parts[0].instrument.midi_channel, Some(2));
}
