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

//! Facade wiring of the P1 SVG output (feature `engrave`).

#![cfg(feature = "engrave")]

use scoretk::{read, write, Format, Options};

#[test]
fn svg_output_is_a_page_display_list() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/v4/changevisibility.mscx"
    );
    let bytes = std::fs::read(path).expect("corpus .mscx");
    let score = read(&bytes, Format::Mscx).expect("lecture MuseScore");
    let svg =
        String::from_utf8(write(&score, Format::Svg, &Options::default()).expect("écriture SVG"))
            .expect("UTF-8");
    assert!(svg.contains("class=\"StaffLines\""));
    assert!(svg.contains("class=\"Note\""));
    assert!(svg.contains("class=\"Stem\""));
    assert!(svg.contains("class=\"BarLine\""));
    assert!(svg.contains("<svg"));
}

#[test]
fn svg_is_read_only_as_a_format_without_reader() {
    assert!(!Format::Svg.supports_read());
    assert!(Format::Svg.supports_write());
}
