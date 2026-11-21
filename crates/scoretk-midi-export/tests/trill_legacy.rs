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

//! Trille 2.x et ornements d'agrément fusionnés.
//!
//! `articulationNames2SymId206` (`engraving/rw/read206/read206.cpp:615-642`)
//! traduit les anciens noms d'articulation : `<subtype>trill</subtype>` est
//! `SymId::ornamentTrill`. Le port ne reconnaissait que les noms SMuFL
//! (`ornamentTrill`) : `Vingt_danses` (2.06) ne jouait donc pas ses trilles.
//!
//! `graceNotesMerged` (`compatmidirenderinternal.cpp:1702-1718`) : quand un
//! accord porte un trille, les ornements d'agrément sont déjà fusionnés dans
//! l'excursion ; `createGraceNotesPlayEvents` (`compatmidirender.cpp:531-541`)
//! ne les émet pas, et `renderNoteArticulation` réserve seulement leurs places
//! (`gnb + p + b + s + gna`).
//!
//! Valeurs attendues : MuseScore 4.7.4 sur les deux `.mscx` 2.06 ci-dessous.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

fn score(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="2.06">
  <Score>
    <Division>480</Division>
    <Part><Staff id="1"/><trackName>Piano</trackName>
      <Instrument id="keyboard.piano"><longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId><Channel><program value="0"/></Channel></Instrument></Part>
    <Staff id="1">
      <Measure number="1">
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        {body}
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
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

fn blanche_trillee() -> &'static str {
    r#"<Chord><durationType>half</durationType><Articulation><subtype>trill</subtype></Articulation>
          <Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
        <Chord><durationType>half</durationType><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>"#
}

#[test]
fn le_nom_2x_trill_joue_le_trille() {
    let mut attendu = Vec::new();
    for (index, tick) in [
        0u32, 59, 120, 179, 240, 299, 360, 419, 480, 539, 600, 659, 720, 779, 840, 899,
    ]
    .into_iter()
    .enumerate()
    {
        attendu.push((if index % 2 == 0 { 72 } else { 74 }, tick));
    }
    attendu.push((74, 960));
    assert_eq!(attaques(&score(blanche_trillee())), attendu);
}

#[test]
fn une_grace_avant_un_trille_est_fusionnee() {
    // La grâce 71 ne joue pas ; le trille perd une place de 32e à la fin
    // (dernière attaque à 779 au lieu de 899).
    let corps = r#"<Chord><durationType>16th</durationType><grace16/><Note><pitch>71</pitch><tpc>13</tpc></Note></Chord>
        <Chord><durationType>half</durationType><Articulation><subtype>trill</subtype></Articulation>
          <Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>
        <Chord><durationType>half</durationType><Note><pitch>74</pitch><tpc>16</tpc></Note></Chord>"#;
    let mut attendu = Vec::new();
    for (index, tick) in [
        0u32, 59, 120, 179, 240, 299, 360, 419, 480, 539, 600, 659, 720, 779,
    ]
    .into_iter()
    .enumerate()
    {
        attendu.push((if index % 2 == 0 { 72 } else { 74 }, tick));
    }
    attendu.push((74, 960));
    assert_eq!(attaques(&score(corps)), attendu);
}
