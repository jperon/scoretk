// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `scm/midi.scm` (`instrument-names-alist`, `midi-program`,
// `percussion?`) et de `lily/staff-performer.cc` (`new_instrument_string`) de
// LilyPond 2.26.0, Copyright (C) 1998--2026 Han-Wen Nienhuys et al.,
// GPL-3.0-or-later ; table réécrite en Rust en 2026. Aucun fichier LilyPond
// n'est distribué : seuls les noms et les programmes GM ont été relevés.
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

//! Table `midiInstrument` → programme General MIDI.
//!
//! `lily/staff-performer.cc:240-252` lit la propriété `midiInstrument` de
//! `Staff` comme une chaîne ; `scm/midi.scm:217-224` (`midi-program`) la
//! résout dans `instrument-names-alist` (valeurs 1..=128, moins un pour le
//! programme base zéro de MIDI). Les entrées 32768+N sont les kits de
//! percussion (`percussion?`, `scm/midi.scm:210-215`) : elles gardent le
//! programme N mais jouent sur le canal 9.

/// Programme MIDI base zéro et canal de percussion d'un `midiInstrument`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MidiInstrument {
    pub program: u8,
    pub percussion: bool,
}

/// `scm/midi.scm` `instrument-names-alist` : (nom, programme base zéro, kit).
const NAMES: &[(&str, u8, bool)] = &[
    ("acoustic grand", 0, false),
    ("bright acoustic", 1, false),
    ("electric grand", 2, false),
    ("honky-tonk", 3, false),
    ("electric piano 1", 4, false),
    ("electric piano 2", 5, false),
    ("harpsichord", 6, false),
    ("clav", 7, false),
    ("celesta", 8, false),
    ("glockenspiel", 9, false),
    ("music box", 10, false),
    ("vibraphone", 11, false),
    ("marimba", 12, false),
    ("xylophone", 13, false),
    ("tubular bells", 14, false),
    ("dulcimer", 15, false),
    ("drawbar organ", 16, false),
    ("percussive organ", 17, false),
    ("rock organ", 18, false),
    ("church organ", 19, false),
    ("reed organ", 20, false),
    ("accordion", 21, false),
    ("harmonica", 22, false),
    ("concertina", 23, false),
    ("acoustic guitar (nylon)", 24, false),
    ("acoustic guitar (steel)", 25, false),
    ("electric guitar (jazz)", 26, false),
    ("electric guitar (clean)", 27, false),
    ("electric guitar (muted)", 28, false),
    ("overdriven guitar", 29, false),
    ("distorted guitar", 30, false),
    ("guitar harmonics", 31, false),
    ("acoustic bass", 32, false),
    ("electric bass (finger)", 33, false),
    ("electric bass (pick)", 34, false),
    ("fretless bass", 35, false),
    ("slap bass 1", 36, false),
    ("slap bass 2", 37, false),
    ("synth bass 1", 38, false),
    ("synth bass 2", 39, false),
    ("violin", 40, false),
    ("viola", 41, false),
    ("cello", 42, false),
    ("contrabass", 43, false),
    ("tremolo strings", 44, false),
    ("pizzicato strings", 45, false),
    ("orchestral harp", 46, false),
    ("timpani", 47, false),
    ("string ensemble 1", 48, false),
    ("string ensemble 2", 49, false),
    ("synthstrings 1", 50, false),
    ("synthstrings 2", 51, false),
    ("choir aahs", 52, false),
    ("voice oohs", 53, false),
    ("synth voice", 54, false),
    ("orchestra hit", 55, false),
    ("trumpet", 56, false),
    ("trombone", 57, false),
    ("tuba", 58, false),
    ("muted trumpet", 59, false),
    ("french horn", 60, false),
    ("brass section", 61, false),
    ("synthbrass 1", 62, false),
    ("synthbrass 2", 63, false),
    ("soprano sax", 64, false),
    ("alto sax", 65, false),
    ("tenor sax", 66, false),
    ("baritone sax", 67, false),
    ("oboe", 68, false),
    ("english horn", 69, false),
    ("bassoon", 70, false),
    ("clarinet", 71, false),
    ("piccolo", 72, false),
    ("flute", 73, false),
    ("recorder", 74, false),
    ("pan flute", 75, false),
    ("blown bottle", 76, false),
    ("shakuhachi", 77, false),
    ("whistle", 78, false),
    ("ocarina", 79, false),
    ("lead 1 (square)", 80, false),
    ("lead 2 (sawtooth)", 81, false),
    ("lead 3 (calliope)", 82, false),
    ("lead 4 (chiff)", 83, false),
    ("lead 5 (charang)", 84, false),
    ("lead 6 (voice)", 85, false),
    ("lead 7 (fifths)", 86, false),
    ("lead 8 (bass+lead)", 87, false),
    ("pad 1 (new age)", 88, false),
    ("pad 2 (warm)", 89, false),
    ("pad 3 (polysynth)", 90, false),
    ("pad 4 (choir)", 91, false),
    ("pad 5 (bowed)", 92, false),
    ("pad 6 (metallic)", 93, false),
    ("pad 7 (halo)", 94, false),
    ("pad 8 (sweep)", 95, false),
    ("fx 1 (rain)", 96, false),
    ("fx 2 (soundtrack)", 97, false),
    ("fx 3 (crystal)", 98, false),
    ("fx 4 (atmosphere)", 99, false),
    ("fx 5 (brightness)", 100, false),
    ("fx 6 (goblins)", 101, false),
    ("fx 7 (echoes)", 102, false),
    ("fx 8 (sci-fi)", 103, false),
    ("sitar", 104, false),
    ("banjo", 105, false),
    ("shamisen", 106, false),
    ("koto", 107, false),
    ("kalimba", 108, false),
    ("bagpipe", 109, false),
    ("fiddle", 110, false),
    ("shanai", 111, false),
    ("tinkle bell", 112, false),
    ("agogo", 113, false),
    ("steel drums", 114, false),
    ("woodblock", 115, false),
    ("taiko drum", 116, false),
    ("melodic tom", 117, false),
    ("synth drum", 118, false),
    ("reverse cymbal", 119, false),
    ("guitar fret noise", 120, false),
    ("breath noise", 121, false),
    ("seashore", 122, false),
    ("bird tweet", 123, false),
    ("telephone ring", 124, false),
    ("helicopter", 125, false),
    ("applause", 126, false),
    ("gunshot", 127, false),
    ("standard kit", 0, true),
    ("standard drums", 0, true),
    ("drums", 0, true),
    ("room kit", 8, true),
    ("room drums", 8, true),
    ("power kit", 16, true),
    ("power drums", 16, true),
    ("rock drums", 16, true),
    ("electronic kit", 24, true),
    ("electronic drums", 24, true),
    ("tr-808 kit", 25, true),
    ("tr-808 drums", 25, true),
    ("jazz kit", 32, true),
    ("jazz drums", 32, true),
    ("brush kit", 40, true),
    ("brush drums", 40, true),
    ("orchestra kit", 48, true),
    ("orchestra drums", 48, true),
    ("classical drums", 48, true),
    ("sfx kit", 56, true),
    ("sfx drums", 56, true),
    ("mt-32 kit", 127, true),
    ("mt-32 drums", 127, true),
    ("cm-64 kit", 127, true),
    ("cm-64 drums", 127, true),
];

/// Résout un nom d'instrument LilyPond (comparaison insensible à la casse).
pub(crate) fn lookup(name: &str) -> Option<MidiInstrument> {
    let name = name.trim();
    NAMES
        .iter()
        .find(|(candidat, _, _)| candidat.eq_ignore_ascii_case(name))
        .map(|(_, program, percussion)| MidiInstrument {
            program: *program,
            percussion: *percussion,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_des_programmes_et_des_kits() {
        assert_eq!(
            lookup("acoustic grand"),
            Some(MidiInstrument {
                program: 0,
                percussion: false
            })
        );
        assert_eq!(lookup("Flute").map(|i| i.program), Some(73));
        assert_eq!(lookup("  clarinet ").map(|i| i.program), Some(71));
        assert_eq!(lookup("gunshot").map(|i| i.program), Some(127));
        assert_eq!(
            lookup("rock drums"),
            Some(MidiInstrument {
                program: 16,
                percussion: true
            })
        );
        assert_eq!(lookup("inconnu"), None);
        assert_eq!(NAMES.len(), 153);
    }
}
