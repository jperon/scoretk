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

//! ABC 2.1 §11 : `T:`/`C:` ne couvrent qu'une partie des métadonnées du
//! modèle ; une directive `%%scoretk meta` privée transporte les autres.

use scoretk::model::{Fraction, Measure, Part, Score};
use scoretk::{read, write, Format, Options};

#[test]
fn metadonnees_complementaires_vont_et_reviennent_par_abc() {
    let mut source = Score::new();
    let part = source.add_part(Part::new("Piano", "Pno"));
    source.add_staff(part);
    source.add_measure(Measure::new(0, Fraction::ZERO, Fraction::ONE));
    source.metadata.work_title = "Œuvre".to_owned();
    source.metadata.subtitle = "Sous-titre".to_owned();
    source.metadata.lyricist = "Parolier".to_owned();
    source.metadata.copyright = "copyright 2026".to_owned();

    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(
        text.contains("%%scoretk meta subtitle Sous-titre\n"),
        "{text}"
    );
    assert!(
        text.contains("%%scoretk meta lyricist Parolier\n"),
        "{text}"
    );
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    assert_eq!(back.metadata.subtitle, "Sous-titre");
    assert_eq!(back.metadata.lyricist, "Parolier");
    assert_eq!(back.metadata.copyright, "copyright 2026");
    assert_eq!(back.metadata.work_title, "Œuvre");
}
