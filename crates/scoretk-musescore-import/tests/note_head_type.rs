// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `TRead::read(Note*)` (`engraving/rw/read400/tread.cpp:3110-3111`) :
// `<headType>` porte la tête résolue (`Pid::HEAD_TYPE`). MuseScore 3.6
// l'écrit déjà en clair (`<headType>whole</headType>`), et
// `writeNotehead` (`exportmusicxml.cpp:3898-3902`) en tire
// `filled="no">normal</notehead>`.

use scoretk_core::model::{ChordRestKind, NoteHeadType, SegmentItem};
use scoretk_musescore_import::MsczContainer;

const SCORE: &str = r#"<museScore version="3.02"><Score>
<Division>480</Division>
<Part><Staff id="1"/><trackName>Voix</trackName>
  <Instrument id="voice.soprano"><longName>Voix</longName>
    <trackName>Voix</trackName><instrumentId>voice.soprano</instrumentId>
    <Channel><program value="52"/></Channel></Instrument></Part>
<Staff id="1">
  <Measure><voice>
    <Chord><durationType>half</durationType>
      <Note><pitch>64</pitch><tpc>14</tpc><headType>whole</headType></Note>
    </Chord>
  </voice></Measure>
</Staff>
</Score></museScore>
"#;

#[test]
fn head_type_3x_est_lu() {
    let container = MsczContainer::from_bytes(SCORE.as_bytes()).expect("conteneur 3.x");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture 3.x");
    let head = score.measures[0]
        .segments
        .iter()
        .flat_map(|segment| segment.iter_elements())
        .find_map(|(_, item)| match item {
            SegmentItem::ChordRest(chord_rest) => match &chord_rest.kind {
                ChordRestKind::Chord(chord) => chord.notes.first().map(|id| score.note(*id).head),
                ChordRestKind::Rest(_) => None,
            },
            _ => None,
        })
        .expect("note lue");
    assert_eq!(head, NoteHeadType::Whole);
}
