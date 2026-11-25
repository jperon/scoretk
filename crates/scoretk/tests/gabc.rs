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

use scoretk::{read, write, Error, Format, Options};

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
fn gabc_round_trip_preserves_the_source_bytes() {
    let score = read(GABC.as_bytes(), Format::Gabc).expect("GABC import");
    let bytes = write(&score, Format::Gabc, &Options::default()).expect("GABC export");
    assert_eq!(String::from_utf8(bytes).expect("UTF-8"), GABC);
    let again = read(GABC.as_bytes(), Format::Gabc).expect("second import");
    assert_eq!(score, again);
}

#[test]
fn writing_gabc_without_a_gregorian_payload_is_an_error() {
    let source =
        br#"<museScore version="4.60"><Score><Division>480</Division></Score></museScore>"#;
    let score = read(source, Format::Mscx).expect("MuseScore import");
    let error = write(&score, Format::Gabc, &Options::default()).expect_err("no payload");
    assert!(matches!(error, Error::Gabc(_)), "{error}");
}
