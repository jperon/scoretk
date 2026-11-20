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

//! Écriture du port MIDI du canal d'un instrument.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const InstrChannel*)` (`write/twrite.cpp:2067-2122`) écrit
/// `<midiPort>` dans `<Channel>` quand l'attribution MIDI est exportée
/// (`:2111-2114`). Sans lui, MuseScore recalcule le port depuis le canal à la
/// réouverture et l'export MusicXML perd le `<midi-device port>` que scoretk
/// tire directement de `Instrument::midi_port`.
#[test]
fn port_midi_du_canal_ecrit_et_relu() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("voices.mscx")).expect("corpus");
    let mut partition = lire(&source);
    assert!(
        !partition.parts.is_empty(),
        "le corpus doit avoir une partie"
    );
    partition.parts[0].instrument.midi_port = Some(1);

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<midiPort>1</midiPort>"),
        "le port du premier canal doit être réécrit dans <Channel>"
    );
    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.parts[0].instrument.midi_port, Some(1));

    // Cas non concerné : sans port, aucun <midiPort> n'est inventé.
    partition.parts[0].instrument.midi_port = None;
    let sans_port = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        !sans_port.contains("<midiPort>1</midiPort>"),
        "un port absent ne doit pas être inventé"
    );
    assert_eq!(
        lire(sans_port.as_bytes()).parts[0].instrument.midi_port,
        None
    );
}
