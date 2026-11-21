// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/compat/midi/compatmidirenderinternal.cpp`
// (`renderHarmony`, 1021-1078) et de `engraving/dom/realizedharmony.cpp`
// (`generateNotes` 92-180, `getIntervals` 292+, `getActualDuration` 242+)
// de MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Lecture MIDI des accords chiffrés (`<Harmony>`).
//!
//! Attendus mesurés : partition réduite à deux mesures, `Dm` puis `A7`,
//! exportée par MuseScore 4.7.4 (`mscore -o ref.mid`). Le style par défaut
//! donne un voicing `AUTO` (proche) : basse à l'octave 3, fondamentale et
//! degrés à l'octave 5, durée jusqu'au symbole suivant. Aucune fixture
//! binaire n'est versionnée.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn notes(bytes: &[u8]) -> Vec<(i64, u8, i64)> {
    let smf = Smf::parse(bytes).expect("SMF valide");
    let mut result = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        let mut open: Vec<(u8, i64)> = Vec::new();
        for event in track {
            tick += i64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                let key = key.as_int();
                if vel.as_int() > 0 {
                    open.push((key, tick));
                } else if let Some(position) = open.iter().position(|(pitch, _)| *pitch == key) {
                    let (pitch, start) = open.remove(position);
                    result.push((start, pitch, tick - start));
                }
            }
        }
        assert!(open.is_empty(), "note non éteinte");
    }
    result.sort_unstable();
    result
}

fn mscx() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure><voice>
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
        <Harmony><root>16</root><name>m</name></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </voice></Measure>
      <Measure><voice>
        <Harmony><root>17</root><name>7</name></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
      </voice></Measure>
    </Staff>
  </Score>
</museScore>
"#
    .to_owned()
}

fn mscx_desactive() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure><voice>
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
        <Harmony><root>16</root><name>m</name><play>0</play></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </voice></Measure>
    </Staff>
  </Score>
</museScore>
"#
    .to_owned()
}

/// `<play>0</play>` coupe la réalisation (`renderHarmony` teste
/// `h->play()`, `compatmidirenderinternal.cpp:1196-1208`) : seule la note
/// écrite sonne, alors que la partition de Marusia en porte 23 ainsi.
#[test]
fn accord_chiffre_desactive_ne_joue_pas() {
    let container = MsczContainer::from_bytes(mscx_desactive().as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let midi = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    assert_eq!(notes(&midi), vec![(0, 60, 1919)]);
}

fn mscx_bemol() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure><voice>
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
        <Harmony><root>12</root></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </voice></Measure>
    </Staff>
  </Score>
</museScore>
"#
    .to_owned()
}

/// `root` 12 vaut si bémol (`TPC_B_B`) : la classe de hauteur se lit sur le
/// cercle des quintes, y compris sous fa (`tpc2pitch`, `pitch.cpp`).
/// MuseScore 4.7.4 joue basse 46, puis 62, 65, 70.
#[test]
fn accord_chiffre_sur_une_fondamentale_bemol() {
    let container = MsczContainer::from_bytes(mscx_bemol().as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let midi = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let got = notes(&midi);
    let expected: Vec<(i64, u8, i64)> = vec![
        (0, 46, 1920), // basse si bémol (octave 3)
        (0, 60, 1919), // note écrite
        (0, 62, 1920), // ré
        (0, 65, 1920), // fa
        (0, 70, 1920), // fondamentale si bémol (octave 5)
    ];
    assert_eq!(got, expected, "accord chiffré sur si bémol");
}

#[test]
fn accord_chiffre_joue_avec_le_voicing_proche() {
    let container = MsczContainer::from_bytes(mscx().as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let midi = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let got = notes(&midi);
    let expected: Vec<(i64, u8, i64)> = vec![
        (0, 38, 1920),    // basse Dm (octave 3)
        (0, 60, 1919),    // note écrite
        (0, 62, 1920),    // fondamentale D (octave 5)
        (0, 65, 1920),    // tierce mineure F
        (0, 69, 1920),    // quinte A
        (1920, 45, 1920), // basse A7
        (1920, 61, 1920), // do dièse
        (1920, 62, 1919), // note écrite
        (1920, 64, 1920), // mi
        (1920, 67, 1920), // sol
        (1920, 69, 1920), // fondamentale A
    ];
    assert_eq!(got, expected, "accord chiffré joué");
}
