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
//
// Portage de `InstrChannel` (`engraving/dom/instrument.h:79-139`) et de
// `TRead::read(InstrChannel*)` (`read400/tread.cpp:926-1020`,
// `read500/tread.cpp:1144-1238`) de MuseScore Studio, modifié (réécrit en
// Rust) en 2026.

//! Canaux MIDI déclarés par un instrument (`<Channel>` d'un `.mscx`).
//!
//! `Instrument::channels()` (`instrument.h:319`) porte tous les canaux d'un
//! instrument : `normal`, `pizzicato`, `mute`, `open`, `arco`… Chacun reçoit
//! un numéro MIDI par `MasterScore::updateMidiMapping`
//! (`midimapping.cpp:259-340`), explicite dans le fichier via `<midiPort>` /
//! `<midiChannel>` ou alloué librement. L'export MIDI écrit le restrike sur
//! **chaque** canal de la partie (`exportmidi.cpp:248-255,304-308`), d'où la
//! nécessité de conserver la table complète et pas seulement le premier canal.

/// Un `<Channel>` d'instrument, tel que lu dans le fichier source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstrumentChannel {
    /// `InstrChannel::name()` (`instrument.h:100`). Chaîne vide quand le
    /// fichier n'écrit pas d'attribut `name` : le C++ lui substitue alors
    /// `InstrChannel::DEFAULT_NAME` (`"normal"`, `instrument.h:82`), que
    /// `TWrite::write` omet à nouveau (`write/twrite.cpp:2069-2073`).
    pub name: String,
    /// Programme MIDI (`InstrChannel::program()`, `instrument.h:119`), base
    /// zéro. `-1` dans le C++ quand le fichier ne le fixe pas.
    pub program: Option<u8>,
    /// Banque MIDI (`InstrChannel::bank()`, `instrument.h:121`), composée par
    /// `TRead` des contrôleurs 0 (HBANK) et 32 (LBANK) (`tread.cpp:953-961`).
    pub bank: Option<u16>,
    /// Port MIDI explicite (`<midiPort>`, `tread.cpp:1002-1005`).
    pub midi_port: Option<u8>,
    /// Canal MIDI explicite (`<midiChannel>`, base zéro, `tread.cpp:1004-1005`).
    pub midi_channel: Option<u8>,
    /// `<mute>` du canal (`tread.cpp:998-999`).
    pub mute: Option<i32>,
    /// `<solo>` du canal (`tread.cpp:1000-1001`).
    pub solo: Option<i32>,
}
