// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/layoutbreak.cpp:43, 209-210` (`Pid::PAUSE`,
// défaut `Sid::sectionPause` = 3 s) et du repli des pauses par segment de
// `engraving/dom/tempotimeline.cpp:134-180`. Une rupture de section portée
// par la dernière mesure d'un passage rejoué par un saut est rejouée à la
// frontière du saut.
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

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

const SOURCE: &str = include_str!("section_pause_jump.mscx");

#[test]
fn pause_de_section_rejouee_au_saut() {
    let container = MsczContainer::from_bytes(SOURCE.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture");
    let midi = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&midi).expect("SMF valide");
    let mut onsets = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
        for event in track {
            tick += i64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { vel, .. },
                ..
            } = event.kind
            {
                if vel.as_int() > 0 {
                    onsets.push(tick / 480);
                }
            }
        }
    }
    onsets.sort_unstable();
    // MIDI MuseScore 4.7.4 : deux passages (0–7, 12–19), une pause de 4
    // noires après chacun (8–11, 20–23), puis les trois dernières mesures.
    let expected: Vec<i64> = [
        0, 1, 2, 3, 4, 5, 6, 7, 12, 13, 14, 15, 16, 17, 18, 19, 24, 28, 29,
    ]
    .into_iter()
    .collect();
    assert_eq!(onsets, expected, "pause de section au saut");
}
