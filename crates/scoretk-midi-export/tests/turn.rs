// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! `ornamentTurn` is realised by MuseScore as four 32nd notes
//! `{+1, 0, -1, 0}` around the main pitch (`compatmidirenderinternal.h:88`),
//! sustained to fill the note (`sustainp=true`,
//! `compatmidirender.cpp:665-800`). The Rust writer played only the main
//! note, losing three notes (Buenos_dias,_Mexico_!.mscz).

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
          <Chord><durationType>16th</durationType>
            <Articulation><subtype>ornamentTurn</subtype></Articulation>
            <Note><pitch>66</pitch><tpc>20</tpc></Note></Chord>
          <Chord><durationType>eighth</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
}

/// Couples (hauteur, attaque) des notes du SMF produit, en ticks.
fn attaques(mscx: &str) -> Vec<(u8, u32)> {
    let conteneur = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF valide");
    let mut resultat = Vec::new();
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
                    resultat.push((key.as_int(), tick));
                }
            }
        }
    }
    resultat.sort_by_key(|(_, attaque)| *attaque);
    resultat
}

#[test]
fn ornement_turn_joue_quatre_notes() {
    // 16th = 120 ticks ; four 32nds at 0/30/60/90 ticks, plus the next note.
    assert_eq!(
        attaques(mscx()),
        vec![(67, 0), (66, 30), (64, 60), (66, 90), (64, 120)]
    );
}
