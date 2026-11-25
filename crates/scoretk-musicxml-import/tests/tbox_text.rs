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

//! Textes de cadre réduits de `v4/repeat65` et `v4/voiceswitching` : les fins
//! de ligne et les lignes suivantes font partie du texte du cadre, et les
//! guillemets restent échappés comme dans le `.mscx`.

use scoretk_musicxml_import::read_musicxml;

fn score(words: &str) -> scoretk_core::model::Score {
    let xml = format!(
        r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions></attributes>
    <direction placement="above"><direction-type>{words}</direction-type></direction>
  </measure></part>
</score-partwise>"#
    );
    read_musicxml(xml.as_bytes()).expect("lecture")
}

#[test]
fn cadre_garde_la_fin_de_ligne() {
    let score = score(
        r#"<words valign="top">no playback
</words><words></words>"#,
    );
    assert_eq!(score.engraving.tbox_texts.len(), 1);
    assert_eq!(score.engraving.tbox_texts[0].text.plain(), "no playback\n");
}

#[test]
fn cadre_concatene_les_lignes_suivantes() {
    let score = score(
        r#"<words valign="top">first line
</words><words>second &quot;quoted&quot; line
</words><words>last line</words>"#,
    );
    assert_eq!(score.engraving.tbox_texts.len(), 1);
    assert_eq!(
        score.engraving.tbox_texts[0].text.plain(),
        "first line\nsecond &quot;quoted&quot; line\nlast line"
    );
}
