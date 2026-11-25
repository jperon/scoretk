// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : tests du branchement MEI dans la façade.
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

use scoretk::{read, write, Format, Options};

const MEI: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
  <meiHead><fileDesc><titleStmt><title>Tiny</title></titleStmt>
    <pubStmt><date isodate="2026-01-01T00:00:00"/></pubStmt></fileDesc></meiHead>
  <music><body><mdiv><score>
    <scoreDef><staffGrp><staffDef n="1" lines="5" meter.count="4" meter.unit="4">
      <clef shape="G" line="2"/></staffDef></staffGrp></scoreDef>
    <section><measure n="1"><staff n="1"><layer n="1">
      <note dur="4" pname="c" oct="4"/><note dur="4" pname="e" oct="4"/>
      <rest dur="2"/>
    </layer></staff></measure></section>
  </score></mdiv></body></music>
</mei>"#;

#[test]
fn mei_is_detected_by_extension_and_namespace() {
    assert_eq!(Format::from_extension("mei"), Some(Format::Mei));
    assert_eq!(Format::detect(MEI), Some(Format::Mei));
    assert!(Format::Mei.supports_read());
    assert!(Format::Mei.supports_write());
}

#[test]
fn mei_round_trip_through_the_facade() {
    let score = read(MEI, Format::Mei).expect("MEI import");
    assert_eq!(score.metadata.work_title, "Tiny");
    assert_eq!(score.notes.len(), 2);
    let bytes = write(&score, Format::Mei, &Options::default()).expect("MEI export");
    let text = String::from_utf8(bytes).expect("UTF-8 MEI");
    assert!(text.contains("xmlns=\"http://www.music-encoding.org/ns/mei\""));
    assert!(text.contains("<note"));
    let again = read(text.as_bytes(), Format::Mei).expect("MEI re-import");
    assert_eq!(again.notes.len(), 2);
    assert_eq!(again.notes[0].pitch, score.notes[0].pitch);
}
