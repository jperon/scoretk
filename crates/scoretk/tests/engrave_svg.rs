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

fn corpus(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("lecture de {} : {error}", path.display()))
}

#[test]
fn svg_groups_expose_system_measure_note_and_lyric_ids() {
    let score = read(&corpus("v4/voiceswitching.mscx"), Format::Mscx).expect("lecture MuseScore");
    let svg = String::from_utf8(write(&score, Format::Svg, &Options::default()).expect("SVG"))
        .expect("UTF-8");
    assert!(svg.contains("<g class=\"system\" data-system=\"0\">"));
    assert!(svg.contains("<g class=\"measure\" id=\"p0-m0\" data-ordinal=\"0\">"));
    assert!(svg.contains("<g class=\"staff\" data-staff=\"0\">"));
    assert!(svg.contains("<g class=\"note\" id=\"m0-s0-v0-c0-n0\""));
    assert!(svg.contains("class=\"Note\""));
    assert!(
        svg.matches("<g ").count() == svg.matches("</g>").count(),
        "groupes non équilibrés"
    );
}

#[test]
fn layout_json_is_joignable_au_temps_et_a_la_page() {
    let score = read(&corpus("v4/voiceswitching.mscx"), Format::Mscx).expect("lecture MuseScore");
    let json =
        String::from_utf8(scoretk::write_layout(&score, 0).expect("layout JSON")).expect("UTF-8");
    assert!(json.contains("\"unit\":\"1/1024 staff space\""));
    assert!(json.contains("\"ordinal\":0"));
    assert!(json.contains("\"size\":[122880,173744]"));
    let id = json
        .split("\"id\":\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("identifiant de note");
    assert_eq!(
        scoretk::page_of_element(&score, id).expect("recherche"),
        Some(0)
    );
    assert_eq!(
        scoretk::page_of_element(&score, "m0-s0-v0-c999-n999").expect("recherche"),
        None
    );
}
