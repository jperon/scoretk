// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
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

//! Clef par défaut d'une portée : `<defaultClef>` et `<clef>` d'instrument.
//!
//! Cas signalé : une portée de ténor sans `<Clef>` dans les mesures, dont le
//! `.mscx` déclare `<defaultClef>G8vb</defaultClef>` et une clef d'instrument
//! `G8vb`. MuseScore exporte alors `<sign>G</sign><line>2</line>` avec un
//! `<clef-octave-change>-1</clef-octave-change>`.
//!
//! Les deux lecteurs (3.x et 4.x) sont couverts, y compris le repli sur la
//! clef de l'instrument et les clefs concert/transposées.

use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};

/// Date d'encodage figée pour une sortie reproductible.
const ENCODING_DATE: &str = "2026-01-01";

/// `.mscx` minimal : une partie de ténor, une mesure, une ronde.
///
/// `staff_default` reçoit la ou les balises `<defaultClef>` de la déclaration
/// de portée, `instrument_clefs` celles de l'instrument.
fn mscx(version: &str, staff_default: &str, instrument_clefs: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="{version}">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
{staff_default}      </Staff>
      <trackName>Tenor</trackName>
      <Instrument id="voice.tenor">
        <longName>Tenor</longName>
        <shortName>T.</shortName>
        <trackName>Tenor</trackName>
        <instrumentId>voice.tenor</instrumentId>
{instrument_clefs}        <Channel><program value="52"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
}

fn render(version: &str, staff_default: &str, instrument_clefs: &str, concert: bool) -> String {
    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            concert_pitch: concert,
            ..ExportOptions::default()
        },
    };
    let input = mscx(version, staff_default, instrument_clefs);
    let bytes = convert(input.as_bytes(), &options).expect("conversion du .mscx minimal");
    String::from_utf8(bytes).expect("MusicXML UTF-8")
}

/// Bloc de la première `<clef>` exportée.
fn first_clef(xml: &str) -> &str {
    let start = xml.find("<clef>").expect("aucune clef exportée");
    let end = xml[start..].find("</clef>").expect("clef non fermée") + start + "</clef>".len();
    &xml[start..end]
}

fn assert_clef(xml: &str, sign: &str, line: &str, octave_change: Option<i32>) {
    let clef = first_clef(xml);
    assert!(
        clef.contains(&format!("<sign>{sign}</sign>")),
        "signe {sign} attendu dans {clef}"
    );
    assert!(
        clef.contains(&format!("<line>{line}</line>")),
        "ligne {line} attendue dans {clef}"
    );
    match octave_change {
        Some(change) => assert!(
            clef.contains(&format!(
                "<clef-octave-change>{change}</clef-octave-change>"
            )),
            "changement d'octave {change} attendu dans {clef}"
        ),
        None => assert!(
            !clef.contains("clef-octave-change"),
            "aucun changement d'octave attendu dans {clef}"
        ),
    }
}

/// `<defaultClef>` de portée et `<clef>` d'instrument concordants.
#[test]
fn tenor_8vb_4xx() {
    let xml = render(
        "4.00",
        "        <defaultClef>G8vb</defaultClef>\n",
        "        <clef>G8vb</clef>\n",
        false,
    );
    assert_clef(&xml, "G", "2", Some(-1));
}

/// `<defaultClef>` de portée et `<clef>` d'instrument concordants (3.x).
#[test]
fn tenor_8vb_3xx() {
    let xml = render(
        "3.01",
        "        <defaultClef>G8vb</defaultClef>\n",
        "        <clef>G8vb</clef>\n",
        false,
    );
    assert_clef(&xml, "G", "2", Some(-1));
}

/// Sans `<defaultClef>`, la clef de l'instrument sert de clef par défaut.
#[test]
fn clef_instrument_seule_4xx() {
    let xml = render("4.00", "", "        <clef>G8vb</clef>\n", false);
    assert_clef(&xml, "G", "2", Some(-1));
}

/// Sans `<defaultClef>`, la clef de l'instrument sert de clef par défaut (3.x).
#[test]
fn clef_instrument_seule_3xx() {
    let xml = render("3.01", "", "        <clef>G8vb</clef>\n", false);
    assert_clef(&xml, "G", "2", Some(-1));
}

/// `<concertClef>` / `<transposingClef>` distinctes : la clef transposée sort
/// par défaut, la clef concert avec `concert_pitch`.
#[test]
fn clefs_concert_et_transposees_4xx() {
    let default =
        "        <concertClef>G</concertClef>\n        <transposingClef>G8vb</transposingClef>\n";
    assert_clef(&render("4.00", "", default, false), "G", "2", Some(-1));
    assert_clef(&render("4.00", "", default, true), "G", "2", None);
}

/// Idem en 3.x : les deux moitiés de la paire sont conservées.
#[test]
fn clefs_concert_et_transposees_3xx() {
    let default =
        "        <concertClef>G</concertClef>\n        <transposingClef>G8vb</transposingClef>\n";
    assert_clef(&render("3.01", "", default, false), "G", "2", Some(-1));
    assert_clef(&render("3.01", "", default, true), "G", "2", None);
}
