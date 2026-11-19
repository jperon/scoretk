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

//! Écriture des méta-évènements de paroles `FF 05`.
//!
//! MuseScore 4.7.4 (`exportmidi.cpp`) exporte les paroles du modèle, en
//! ajoutant un espace final aux syllabes `Single`/`End` seulement. Le portage
//! suit ce comportement, contrairement à l'état antérieur qui les ignorait.

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// `.mscx` 4.10 avec quatre noires et une parole par accord.
fn mscx(lyrics: [&str; 3]) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
      </Staff>
      <trackName>Voice</trackName>
      <Instrument id="voice">
        <longName>Voice</longName>
        <trackName>Voice</trackName>
        <instrumentId>voice</instrumentId>
        <Channel><program value="54"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
          <Chord>
            <durationType>quarter</durationType>
            {}
            <Note><pitch>60</pitch><tpc>14</tpc></Note>
          </Chord>
          <Chord>
            <durationType>quarter</durationType>
            {}
            <Note><pitch>62</pitch><tpc>16</tpc></Note>
          </Chord>
          <Chord>
            <durationType>quarter</durationType>
            {}
            <Note><pitch>64</pitch><tpc>17</tpc></Note>
          </Chord>
          <Chord><durationType>quarter</durationType><Note><pitch>65</pitch><tpc>19</tpc></Note></Chord>
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#,
        lyrics[0], lyrics[1], lyrics[2]
    )
}

fn smf(source: &str) -> Vec<u8> {
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture .mscx");
    scoretk_midi_export::write_midi(&score).expect("écriture MIDI")
}

/// `(tick, texte)` des méta-évènements `FF 05`.
fn lyric_events(bytes: &[u8]) -> Vec<(u32, String)> {
    let smf = Smf::parse(bytes).expect("SMF valide");
    let mut result = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0;
        for event in track {
            tick += event.delta.as_int();
            if let TrackEventKind::Meta(MetaMessage::Lyric(text)) = event.kind {
                result.push((tick, String::from_utf8_lossy(text).into_owned()));
            }
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn paroles_ecrites_avec_espace_final_sur_les_syllabes_terminales() {
    let lyrics = [
        "<Lyrics><syllabic>begin</syllabic><text>La</text></Lyrics>",
        "<Lyrics><syllabic>end</syllabic><text>la</text></Lyrics>",
        "<Lyrics><syllabic>single</syllabic><text>Sol</text></Lyrics>",
    ];
    let bytes = smf(&mscx(lyrics));
    assert_eq!(
        lyric_events(&bytes),
        vec![
            (0, "La".to_owned()),
            (480, "la ".to_owned()),
            (960, "Sol ".to_owned()),
        ],
        "FF 05 et espace final des syllabes terminales",
    );

    // Les notes restent à leur place : la musique ordinaire n'est pas modifiée.
    let parsed = Smf::parse(&bytes).expect("SMF valide");
    let mut attacks = Vec::new();
    for track in &parsed.tracks {
        let mut tick = 0;
        for event in track {
            tick += event.delta.as_int();
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                if vel.as_int() != 0 {
                    attacks.push((tick, key.as_int()));
                }
            }
        }
    }
    attacks.sort_unstable();
    assert_eq!(
        attacks,
        vec![(0, 60), (480, 62), (960, 64), (1440, 65)],
        "attaques et hauteurs",
    );

    // Cas non concerné : sans paroles, aucun FF 05 n'est émis.
    assert!(
        lyric_events(&smf(&mscx(["", "", ""]))).is_empty(),
        "aucune parole"
    );
}
