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

//! Vélocité MIDI après `Note::customizeVelocity`.
//!
//! MuseScore applique la personnalisation par note dans `playNote`
//! (`compatmidirenderinternal.cpp:227-229`) : `velo + velo * pct / 100` en mode
//! offset, `velo` absolue en mode user, puis clamp 1–127
//! (`dom/note.cpp:2753-2760`). Le SMF produit est réanalysé avec `midly`.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_core::MsczContainer;

/// `.mscx` 4.10 minimal, avec le contenu de `<voice>` fourni.
fn mscx(voice: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>
    <Part id="1">
      <Staff id="1">
        <StaffType group="pitched"><name>stdNormal</name></StaffType>
      </Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName>
        <shortName>Pno.</shortName>
        <trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      <Measure>
        <voice>
          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
{voice}
        </voice>
      </Measure>
    </Staff>
  </Score>
</museScore>
"#
    )
}

fn chord(notes: &str) -> String {
    format!("          <Chord><durationType>quarter</durationType>{notes}</Chord>\n")
}

fn note(pitch: u8, tpc: i8, extra: &str) -> String {
    format!("<Note><pitch>{pitch}</pitch><tpc>{tpc}</tpc>{extra}</Note>")
}

/// Vélocités d'attaque du SMF, triées par hauteur.
fn velocities(voice: &str) -> Vec<(u8, u8)> {
    let source = mscx(voice);
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = scoretk_core::read::read_score(&container).expect("lecture .mscx");
    let bytes = scoretk_midi::write_midi(&score).expect("écriture MIDI");
    let smf = Smf::parse(&bytes).expect("SMF valide");
    let mut notes = Vec::new();
    for track in &smf.tracks {
        for event in track {
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                if vel.as_int() != 0 {
                    notes.push((key.as_int(), vel.as_int()));
                }
            }
        }
    }
    notes.sort_unstable();
    notes
}

#[test]
fn offset_absolu_et_clamp_sans_nuance() {
    // Sans nuance, `VelocityMap` vaut 80 (`velocitymap.cpp`).
    let voice = [
        chord(&note(
            60,
            14,
            "<velocity>50</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            61,
            14,
            "<velocity>-50</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            62,
            16,
            "<velocity>100</velocity><veloType>offset</veloType>",
        )),
        chord(&note(
            63,
            16,
            "<velocity>42</velocity><veloType>user</veloType>",
        )),
        chord(&note(
            64,
            18,
            "<velocity>0</velocity><veloType>offset</veloType>",
        )),
    ]
    .concat();
    // 80 + 80*50/100 = 120 ; 80 - 40 = 40 ; 80 + 80 = 160 → 127 ;
    // mode user = 42 ; userVelocity 0 = pas de personnalisation (80).
    assert_eq!(
        velocities(&voice),
        vec![(60, 120), (61, 40), (62, 127), (63, 42), (64, 80)]
    );
}

#[test]
fn offset_sur_une_nuance_et_clamp() {
    let voice = [
        "          <Dynamic><subtype>f</subtype><velocity>96</velocity></Dynamic>\n",
        &chord(&note(
            60,
            14,
            "<velocity>50</velocity><veloType>offset</veloType>",
        )),
        &chord(&note(
            61,
            14,
            "<velocity>-50</velocity><veloType>offset</veloType>",
        )),
        &chord(&note(
            62,
            16,
            "<velocity>30</velocity><veloType>user</veloType>",
        )),
    ]
    .concat();
    // 96 + 48 = 144 → 127 ; 96 - 48 = 48 ; mode user = 30.
    assert_eq!(velocities(&voice), vec![(60, 127), (61, 48), (62, 30)]);
}

#[test]
fn deux_notes_du_meme_accord_ont_deux_velocites() {
    let notes = format!(
        "{}{}",
        note(60, 14, "<velocity>50</velocity><veloType>offset</veloType>"),
        note(
            64,
            18,
            "<velocity>-50</velocity><veloType>offset</veloType>"
        )
    );
    // Le facteur est appliqué par note, pas à l'échelle de l'accord.
    assert_eq!(velocities(&chord(&notes)), vec![(60, 120), (64, 40)]);
}

#[test]
fn absence_de_balise_velocity_laisse_la_base() {
    // Aucune balise `<velocity>` : `user_velocity` reste `None` et la vélocité
    // calculée (80 sans nuance) n'est pas modifiée.
    let voice = chord(&note(60, 14, ""));
    assert_eq!(velocities(&voice), vec![(60, 80)]);
}

#[test]
fn ornement_et_tremolo_simple_appliquent_la_velocity() {
    // Ornement : `<acciaccatura/>` rattache l'accord au suivant ; la
    // personnalisation s'applique dans `render_grace_notes`.
    let grace = format!(
        "          <Chord><acciaccatura/><durationType>eighth</durationType>{}</Chord>\n",
        note(60, 14, "<velocity>50</velocity><veloType>offset</veloType>")
    );
    let normal = chord(&note(62, 16, ""));
    let mut ornement = velocities(&[grace, normal].concat());
    ornement.sort_unstable();
    // 80 + 80*50/100 = 120 pour l'ornement ; le normal reste à 80.
    assert_eq!(ornement, vec![(60, 120), (62, 80)]);

    // Trémolo simple : la figure `r8` répète la note ; chaque attaque passe
    // par `customize_velocity` avec la vélocité utilisateur de la note.
    let tremolo = format!(
        "          <Chord><durationType>half</durationType>\
         <TremoloSingleChord><subtype>r8</subtype></TremoloSingleChord>{}</Chord>\n\
         \x20         <Rest><durationType>half</durationType></Rest>\n",
        note(64, 18, "<velocity>50</velocity><veloType>offset</veloType>")
    );
    let repetitions = velocities(&tremolo);
    assert!(
        repetitions.len() >= 2,
        "trémolo simple non répété : {repetitions:?}"
    );
    assert!(
        repetitions
            .iter()
            .all(|(key, vel)| *key == 64 && *vel == 120),
        "vélocité de trémolo incorrecte : {repetitions:?}"
    );
}
