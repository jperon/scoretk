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

//! Facade wiring of GABC: extension, content detection and round trip.

use scoretk::{read, write, Format, Options};

const GABC: &str = "% a comment\nname: Veni Sancte Spiritus;\nlanguage: latin;\nmode: 2;\n%%\n(c3)\nVE(g.)ni(h') (,)\n";

#[test]
fn gabc_is_recognised_by_extension_and_content() {
    assert_eq!(Format::from_extension("gabc"), Some(Format::Gabc));
    assert_eq!(Format::from_extension("GABC"), Some(Format::Gabc));
    assert_eq!(Format::detect(GABC.as_bytes()), Some(Format::Gabc));
    // Comments before `name:` are allowed by Gregorio's examples.
    assert!(GABC.starts_with('%'));
    assert_eq!(Format::detect(b"%abc-2.1\nX:1\n"), Some(Format::Abc));
    assert_eq!(Format::detect(b""), None);
}

#[test]
fn gabc_round_trip_serialises_the_payload() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let bytes = write(&score, Format::Gabc, &Options::default()).expect("GABC export");
    let written = String::from_utf8(bytes).expect("UTF-8");
    // The leading comment is not a modelled field: it must not come back.
    // A byte-identical output would mean the writer copied its input.
    assert!(!written.contains("a comment"), "{written}");
    assert!(
        written.starts_with("name: Veni Sancte Spiritus;\n"),
        "{written}"
    );
    let again = read(written.as_bytes(), Format::Gabc).expect("second import");
    let reference = scoretk::model::inventory(&score);
    let candidate = scoretk::model::inventory(&again);
    let coverage = scoretk::model::compare(&reference, &candidate, scoretk::model::Layers::ALL);
    assert_eq!(
        (
            coverage.total.reference,
            coverage.total.matched,
            coverage.total.extra
        ),
        (reference.items.len(), reference.items.len(), 0),
        "the structured inventory changed"
    );
}
