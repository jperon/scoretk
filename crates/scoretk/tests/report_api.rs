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

//! Additive report API: the old signatures keep their exact output.

use scoretk::model::{Credit, CreditStyle, Layers, RichText};
use scoretk::{read, read_with_report, write, write_with_report, Format, Options};

const MSCX: &[u8] = include_bytes!("../../../tests/corpus/v4/dynamics.mscx");

#[test]
fn read_variants_agree() {
    let (score, report) = read_with_report(MSCX, Format::Mscx).unwrap();
    assert_eq!(score, read(MSCX, Format::Mscx).unwrap());
    assert_eq!(report, score.report);
}

#[test]
fn write_variants_agree_and_report_lost_engraving() {
    let (score, _) = read_with_report(MSCX, Format::Mscx).unwrap();
    for format in [
        Format::MusicXml,
        Format::Mxl,
        Format::Mscx,
        Format::Mscz,
        Format::Ly,
        Format::Abc,
        Format::Midi,
    ] {
        let (bytes, report) = write_with_report(&score, format, &Options::default()).unwrap();
        assert_eq!(bytes, write(&score, format, &Options::default()).unwrap());
        // The read-side report is kept as a prefix; the writer may append its
        // own losses (three `<credit>` blocks are dropped by ABC and MIDI).
        assert!(report.losses.starts_with(&score.report.losses));
    }

    let mut with_credit = score.clone();
    with_credit.add_credit(Credit::new(RichText::new("Titre"), CreditStyle::Title));
    let (_, musicxml) =
        write_with_report(&with_credit, Format::MusicXml, &Options::default()).unwrap();
    assert_eq!(musicxml.lost(Layers::ENGRAVING), 0);
    let (_, abc) = write_with_report(&with_credit, Format::Abc, &Options::default()).unwrap();
    assert_eq!(
        abc.lost(Layers::ENGRAVING),
        with_credit.engraving.credits.len()
    );
}
