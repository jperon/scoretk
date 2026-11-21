// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la table `excursions` (`engraving/compat/midi/compatmidirenderinternal.h`)
// et de `renderNoteArticulation` (`engraving/compat/midi/compatmidirender.cpp`)
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

//! Mordant supérieur court (`ornamentShortTrill`, « prall »).
//!
//! La table `excursions` donne pour `ornamentShortTrill` au style par défaut
//! un corps `{0, 1, 0}`, sans répétition, avec prolongation de la dernière
//! note. `previous_accidental_auxiliary` fournit l'accident de l'auxiliaire.
//!
//! Attendus : MuseScore 4.7.4 sur le `.mscx` 4.10 de ce test (ronde 76, tempo
//! 120, pas de 32e).

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn mscx() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Accordion</trackName>
      <Instrument id="free-reed.accordion"><longName>Accordion</longName><shortName>Acc.</shortName><trackName>Accordion</trackName>
        <instrumentId>free-reed.accordion</instrumentId><Channel><program value="21"/></Channel></Instrument>
    </Part>
    <Staff id="1">
      <Measure><voice>
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Tempo><tempo>2</tempo><followText>1</followText></Tempo>
        <Chord><durationType>whole</durationType>
          <Ornament><subtype>ornamentShortTrill</subtype></Ornament>
          <Note><pitch>76</pitch><tpc>18</tpc></Note>
        </Chord>
      </voice></Measure>
    </Staff>
  </Score>
</museScore>
"#
}

fn attacks(mscx: &str) -> Vec<(i64, u8)> {
    let container = MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&bytes).expect("SMF valide");
    let mut out = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        for event in track {
            tick += i64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                if vel.as_int() > 0 {
                    out.push((tick, key.as_int()));
                }
            }
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn prall_joue_le_corps_0_1_0() {
    assert_eq!(attacks(mscx()), vec![(0, 76), (59, 77), (119, 76)]);
}
