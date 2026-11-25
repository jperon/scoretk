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

//! L'identité d'un instrument ne tient ni dans `%%MIDI program` (numéro GM)
//! ni dans `name=`/`subname=` (noms affichés). ABC 2.1 §11 autorise les
//! directives `%%` privées : `%%scoretk instrument` les complète sans changer
//! le sens des champs standard.

use scoretk::model::{Fraction, Measure, Part, Score};
use scoretk::{read, write, Format, Options};

fn partition() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno"));
    score.parts[part.index()].instrument.id = "keyboard.piano".to_owned();
    score.parts[part.index()].instrument.midi_program = Some(0);
    score.add_staff(part);
    score.add_measure(Measure::new(0, Fraction::ZERO, Fraction::ONE));
    score
}

#[test]
fn identite_instrument_et_noms_de_voix_vont_et_reviennent_par_abc() {
    let source = partition();
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(text.contains("name=\"Piano\""), "{text}");
    assert!(text.contains("snm=\"Pno\""), "{text}");
    assert!(
        text.contains("%%scoretk instrument keyboard.piano\n"),
        "{text}"
    );
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    assert_eq!(back.parts[0].instrument.id, "keyboard.piano");
    assert_eq!(back.parts[0].long_name, "Piano");
    assert_eq!(back.parts[0].short_name, "Pno");
}

#[test]
fn directive_instrument_inconnue_est_ignoree_sans_perdre_la_voix() {
    let score =
        read(b"X:1\nK:C\nV:1\n%%scoretk autre chose\nC4|]\n", Format::Abc).expect("lecture ABC");
    assert_eq!(score.parts.len(), 1);
    assert!(score.parts[0].instrument.id.is_empty());
}
