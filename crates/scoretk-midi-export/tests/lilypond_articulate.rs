// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage des facteurs de `ly/articulate.ly` de LilyPond, Copyright (C)
// 2008--2026 NICTA, auteur Peter Chubb, modifié (réécrit en Rust) en 2026.
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

//! `\articulate` : réduction des notes avant le performeur LilyPond.
//!
//! `ly/articulate.ly` remplace chaque note par une note plus courte et un
//! silence : facteur 7/8 par défaut, sauf articulation explicite ou liaison.
//! `note-performer.cc` joue ensuite ces durées telles quelles.
//!
//! Attendus : LilyPond 2.27 sur la source de ce test (croche = 0,875 noire).

use midly::{MidiMessage, Smf, TrackEventKind};

fn notes(ly: &str) -> Vec<(u8, i64, i64)> {
    let score = scoretk_lilypond_import::read_lilypond(ly).expect("lecture LilyPond");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&bytes).expect("SMF valide");
    let mut open: Vec<(u8, i64)> = Vec::new();
    let mut result = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0i64;
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
                } else if let Some(position) = open.iter().position(|(k, _)| *k == key) {
                    let (k, start) = open.remove(position);
                    result.push((k, start, tick - start));
                }
            }
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn arpege_lilypond_ne_decale_pas_les_notes() {
    let source = "\\version \"2.27.0\"\n\
                  \\score { \\new Staff { <c' e' g'>4\\arpeggio } \\midi{} }\n";
    assert_eq!(
        notes(source),
        vec![(60, 0, 479), (64, 0, 479), (67, 0, 479)]
    );
}

#[test]
fn trille_sans_articulate_n_est_pas_developpe() {
    let source = "\\version \"2.27.0\"\n\
                  \\score { \\new Staff { c'4\\trill } \\midi{} }\n";
    assert_eq!(notes(source), vec![(60, 0, 479)]);
}

#[test]
fn trille_avec_articulate_est_developpe() {
    let source = "\\version \"2.27.0\"\n\
                  \\include \"articulate.ly\"\n\
                  \\score { \\articulate { c'4\\trill } \\midi{} }\n";
    assert!(notes(source).len() > 1);
}

#[test]
fn articulate_reduit_les_notes_non_articulees() {
    let source = "\\version \"2.27.0\"\n\
                  \\include \"articulate.ly\"\n\
                  \\score { \\articulate { c'4 d'2 e'4 } \\midi{} }\n";
    // 7/8 de 480, 960, 720 et 240 ; le -1 est la convention MuseScore du
    // note-off, dans la tolérance de l'oracle.
    assert_eq!(
        notes(source),
        vec![(60, 0, 419), (62, 480, 839), (64, 1440, 419)]
    );
}
