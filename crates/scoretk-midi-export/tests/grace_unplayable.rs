// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! An unplayable grace note must not be rendered. MuseScore's
//! `Chord::graceNotesBefore(true)` / `graceNotesAfter(true)` filter out grace
//! chords whose `isChordPlayable()` is false (`chord.cpp:1383-1400,2121,2142`);
//! the first note's `PLAY` property decides. The Rust writer enumerated every
//! grace chord, adding notes MuseScore does not play (`layout_elements`).

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId><Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord><durationType>16th</durationType><grace16/>
            <Note><pitch>74</pitch><tpc>16</tpc><play>0</play></Note></Chord>
          <Chord><durationType>quarter</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

fn notes() -> Vec<(u8, u32)> {
    let conteneur = MsczContainer::from_bytes(mscx().as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF");
    let mut out = Vec::new();
    for piste in &smf.tracks {
        let mut tick = 0u32;
        for evenement in piste {
            tick += evenement.delta.as_int();
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = evenement.kind
            {
                if vel.as_int() > 0 {
                    out.push((key.as_int(), tick));
                }
            }
        }
    }
    out.sort_by_key(|(_, attaque)| *attaque);
    out
}

#[test]
fn grace_non_jouee_est_ignoree() {
    let found = notes();
    assert!(
        !found.iter().any(|(pitch, _)| *pitch == 74),
        "unplayable grace note was rendered: {found:?}"
    );
    // `graceNotesBefore(true)` filters the whole chord: an unplayable grace
    // must not delay the main note, which stays on the downbeat (tick 0)
    // instead of being pushed to 240 by the appoggiatura span.
    assert!(
        found.iter().any(|(pitch, tick)| *pitch == 60 && *tick == 0),
        "main note shifted by unplayable grace: {found:?}"
    );
    assert!(found.iter().any(|(pitch, _)| *pitch == 62));
}
