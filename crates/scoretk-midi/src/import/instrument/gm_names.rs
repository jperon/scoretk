// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Table dérivée de `importexport/midi/internal/midishared/generalmidi.cpp`
// (`GM1_PROGRAM_DATA`) de MuseScore Studio, Copyright (C) MuseScore Limited
// and others, convertie en Rust (fichier généré par extraction du C++) en
// 2026.
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

//! Noms des programmes General MIDI.
//!
//! Fichier **généré** à partir de `generalmidi.cpp` de MuseScore Studio
//! (`GM1_PROGRAM_DATA`, 128 entrées), comme la table des instruments de
//! `scoretk_core`. Ne pas éditer à la main.

/// Nom de programme General MIDI (`getMidiName`), indexé par programme 0-127.
pub const GM1_NAMES: [&str; 128] = [
    "Grand Piano",
    "Bright Piano",
    "E.Grand",
    "Honky-tonk",
    "E.Piano",
    "E.Piano 2",
    "Harpsichord",
    "Clav.",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Perc. Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Akkordion",
    "Harmonica",
    "Bandoneon",
    "Nylon Gtr.",
    "Steel Gtr.",
    "Jazz Guitar",
    "Clean Guitar",
    "Muted Guitar",
    "Overdrive Gtr",
    "DistortionGtr",
    "Gtr.Harmonics",
    "Acoustic Bass",
    "Fingered Bass",
    "Picked Bass",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Str.",
    "PizzicatoStr.",
    "Harp",
    "Timpani",
    "Strings 1",
    "Strings 2",
    "Syn.Strings 1",
    "Syn.Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Square Wave",
    "Saw Wave",
    "Calliope",
    "Chiffer Lead",
    "Charang",
    "Solo Vox",
    "Fifth Saw",
    "Bass Lead",
    "New Age Pad",
    "Warm Pad",
    "Polysynth Pad",
    "Choir Pad",
    "Bowed Pad",
    "Metallic Pad",
    "Halo Pad",
    "Sweep Pad",
    "Rain",
    "Soundtrack",
    "Crystal",
    "Athmosphere",
    "Brightness",
    "Goblins",
    "Echoes",
    "Sci-Fi",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bagpipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Drum",
    "Synth Drum",
    "Rev. Cymbal",
    "GtrFret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweed",
    "Telephone",
    "Helicopter",
    "Applaus",
    "Gunshot",
];

/// Nom du programme donné, ou « Grand Piano » hors bornes.
pub fn get_midi_name(program: u8) -> &'static str {
    GM1_NAMES
        .get(usize::from(program))
        .copied()
        .unwrap_or("Grand Piano")
}
