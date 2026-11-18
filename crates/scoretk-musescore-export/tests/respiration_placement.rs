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

//! Aller-retour sur le placement des respirations (`<Breath><placement>`),
//! selon la parité de la piste (`track() & 1`).

use scoretk_core::model::{Placement, Score};
use scoretk_musescore_export::write_mscx;
use scoretk_musescore_import::{read, MsczContainer};

const MSCX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.00">
  <programVersion>4.0.0</programVersion>
  <programRevision></programRevision>
  <Score>
    <Division>480</Division>
    <Style><Spatium>1.74978</Spatium></Style>
    <Part>
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
      </Staff>
      <trackName>Piano</trackName>
      <Instrument id="piano">
        <longName>Piano</longName>
        <trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Breath><placement>above</placement></Breath>
        </voice>
      </Measure>
      <Measure>
        <voice>
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Breath><placement>below</placement></Breath>
        </voice>
      </Measure>
      <Measure>
        <voice>
          <Rest><durationType>measure</durationType><duration>4/4</duration></Rest>
        </voice>
        <voice>
          <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>16</tpc></Note></Chord>
          <Breath><placement>above</placement></Breath>
        </voice>
      </Measure>
      <Measure>
        <voice>
          <Rest><durationType>measure</durationType><duration>4/4</duration></Rest>
        </voice>
        <voice>
          <Chord><durationType>quarter</durationType><Note><pitch>64</pitch><tpc>16</tpc></Note></Chord>
          <Breath><placement>below</placement></Breath>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#;

fn read_score(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("container");
    read::read_score(&container).expect("read")
}

fn breath_placements(score: &Score) -> Vec<Placement> {
    let mut out = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_track, item) in segment.iter_elements() {
                if let scoretk_core::model::SegmentItem::Breath(b) = item {
                    out.push(b.placement);
                }
            }
        }
    }
    out
}

#[test]
fn respiration_placement_conserve_et_defaut() {
    let original = read_score(MSCX.as_bytes());
    let placements = breath_placements(&original);
    assert_eq!(
        placements,
        vec![
            Placement::Above,
            Placement::Below,
            Placement::Above,
            Placement::Below
        ]
    );

    let written = write_mscx(&original).expect("write");
    // Placement par défaut omis : voix 1 above et voix 2 below.
    assert_eq!(written.matches("<placement>above</placement>").count(), 1);
    assert_eq!(written.matches("<placement>below</placement>").count(), 1);

    let reread = read_score(written.as_bytes());
    let placements2 = breath_placements(&reread);
    assert_eq!(placements2, placements);
}
