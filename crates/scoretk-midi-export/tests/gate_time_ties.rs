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

//! Durée des liaisons de prolongation sous une durée de tenue d'instrument.
//!
//! Portage de `engraving/compat/midi/compatmidirenderinternal.cpp`
//! (`calculateTieLength`) : la durée ajoutée par chaque note liée est celle de
//! son évènement de jeu, déjà multiplié par le `gateTime` de l'instrument
//! (`compatmidirender.cpp`, `renderChord` : `e.setLen(e.len() * gateTime / 100)`).
//! Une flûte MuseScore a une articulation par défaut de 95 % : deux noires
//! liées doivent donc durer `2 × 480 × 95 / 100 - 1 = 911` ticks, et non 959.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// `.mscx` 4.10 minimal : l'instrument porte une articulation sans nom dont le
/// `gateTime` vaut `gate`, et `mesures` noires liées se suivent.
fn mscx(gate: i32, mesures: usize) -> String {
    let mut corps = String::new();
    for index in 0..mesures {
        let (next, prev) = match (index, mesures) {
            (0, 1) => (String::new(), String::new()),
            (0, _) => (
                String::from("<next><location><measures>1</measures></location></next>"),
                String::new(),
            ),
            (i, _) if i + 1 == mesures => (
                String::new(),
                String::from("<prev><location><measures>-1</measures></location></prev>"),
            ),
            _ => (
                String::from("<next><location><measures>1</measures></location></next>"),
                String::from("<prev><location><measures>-1</measures></location></prev>"),
            ),
        };
        let spanner = if next.is_empty() && prev.is_empty() {
            String::new()
        } else {
            format!("<Spanner type=\"Tie\"><Tie></Tie>{next}{prev}</Spanner>")
        };
        corps.push_str(&format!(
            "      <Measure>\n        <voice>\n\
             {}\
             <Chord><durationType>quarter</durationType>\
             <Note>{spanner}<pitch>60</pitch><tpc>14</tpc></Note></Chord>\n\
             </voice>\n      </Measure>\n",
            if index == 0 {
                "          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>\n"
            } else {
                ""
            }
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
      </Staff>
      <trackName>Flute</trackName>
      <Instrument id="wind.flutes.flute">
        <longName>Flute</longName>
        <shortName>Fl.</shortName>
        <trackName>Flute</trackName>
        <instrumentId>wind.flutes.flute</instrumentId>
        <Articulation>
          <velocity>100</velocity>
          <gateTime>{gate}</gateTime>
          </Articulation>
        <Channel><program value="73"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
{corps}    </Staff>
  </Score>
</museScore>
"#
    )
}

/// Attaque et extinction (ticks) de la première note du SMF produit.
fn premiere_note(mscx: &str) -> (u32, u32) {
    let conteneur = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let partition = scoretk_musescore_import::read::read_score(&conteneur).expect("lecture");
    let octets = scoretk_midi_export::write_midi(&partition).expect("écriture MIDI");
    let smf = Smf::parse(&octets).expect("SMF valide");
    let mut attaque = None;
    for piste in &smf.tracks {
        let mut tick = 0u32;
        for evenement in piste {
            tick += evenement.delta.as_int();
            match evenement.kind {
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, vel },
                    ..
                } if key.as_int() == 60 && vel.as_int() > 0 => {
                    assert!(attaque.is_none(), "une seule attaque attendue");
                    attaque = Some(tick);
                }
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, vel },
                    ..
                } if key.as_int() == 60 && vel.as_int() == 0 && attaque.is_some() => {
                    // L'écrivain note les extinctions en `NoteOn` à vélocité 0.
                    return (attaque.unwrap(), tick);
                }
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOff { key, .. },
                    ..
                } if key.as_int() == 60 && attaque.is_some() => {
                    return (attaque.unwrap(), tick);
                }
                _ => {}
            }
        }
    }
    panic!("extinction de la note 60 introuvable");
}

#[test]
fn liaison_avec_gate_time_d_instrument() {
    // Deux noires liées, flûte (95 %) : 911 ticks. Le code actuel rend 959.
    let (attaque, extinction) = premiere_note(&mscx(95, 2));
    assert_eq!(attaque, 0);
    assert_eq!(extinction, 911);
}

#[test]
fn liaison_sans_gate_time_reste_pleine() {
    // Cas non concerné : gate 100 %, la chaîne de deux noires reste à 959.
    let (attaque, extinction) = premiere_note(&mscx(100, 2));
    assert_eq!(attaque, 0);
    assert_eq!(extinction, 959);
}

#[test]
fn note_isolee_avec_gate_time() {
    // Limite : une seule note, pas de liaison : 480 × 95 / 100 - 1 = 455.
    let (attaque, extinction) = premiere_note(&mscx(95, 1));
    assert_eq!(attaque, 0);
    assert_eq!(extinction, 455);
}
