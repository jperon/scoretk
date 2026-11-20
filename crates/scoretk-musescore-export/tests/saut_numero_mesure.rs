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

//! Propriété `startWithMeasureOne` d'un saut de section.

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn partition(flag_value: &str) -> Score {
    let flag = if flag_value.is_empty() { String::new() } else { format!("<startWithMeasureOne>{flag_value}</startWithMeasureOne>") };
    let mscx = format!(
        r#"<museScore version="4.70"><Score>
  <Division>480</Division>
  <Part id="1"><Instrument><Channel><program value="0"/></Channel></Instrument><Staff id="1"/></Part>
  <Staff id="1">
    <Measure><voice><Chord><durationType>whole</durationType>
      <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice>
      <LayoutBreak><subtype>section</subtype>{flag}</LayoutBreak></Measure>
    <Measure><voice><Chord><durationType>whole</durationType>
      <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></voice></Measure>
    </Staff>
  </Score></museScore>"#
    );
    let container = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `Pid::START_WITH_MEASURE_ONE` (`dom/property.cpp:468`) est lu par
/// `TRead::read(LayoutBreak*)` (`read460/tread.cpp:2971`) et écrit par
/// `TWrite::write(const LayoutBreak*)`. MuseScore s'en sert pour reprendre la
/// numérotation à 1 après un saut de section (`dom/measurebase.cpp:332`).
#[test]
fn saut_de_section_reprend_a_un() {
    let score = partition("1");
    assert!(!score.measures[1].section_keeps_numbering);
    let ecrit = scoretk_musescore_export::write_mscx(&score).expect("écriture");
    assert!(
        ecrit.contains("<subtype>section</subtype>") && !ecrit.contains("<startWithMeasureOne>"),
        "un saut qui relance la numérotation n'écrit pas de propriété (défaut 1)"
    );
    let relu = {
        let container = MsczContainer::from_bytes(ecrit.as_bytes()).expect("conteneur");
        scoretk_musescore_import::read::read_score(&container).expect("relecture")
    };
    assert!(!relu.measures[1].section_keeps_numbering);
    assert_eq!(relu.measures.len(), score.measures.len());
    assert_eq!(relu.notes.len(), score.notes.len());

    // Cas non concerné : un saut sans la propriété n'en invente pas.
    let simple = partition("");
    assert!(!simple.measures[1].section_keeps_numbering);
    let ecrit_simple = scoretk_musescore_export::write_mscx(&simple).expect("écriture");
    assert!(!ecrit_simple.contains("<startWithMeasureOne>"));
    // Cas complémentaire : `startWithMeasureOne="0"` doit être réécrit tel quel.
    let kept = partition("0");
    let ecrit_kept = scoretk_musescore_export::write_mscx(&kept).expect("écriture");
    assert!(ecrit_kept.contains("<startWithMeasureOne>0</startWithMeasureOne>"));
}
