// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/midi/internal/midishared/` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
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

//! Types partagés par la lecture et l'écriture MIDI.
//!
//! Un `-common` ne contient que ce que les deux sens utilisent (règle dans
//! `docs/agents/developpement.md`) : ici, l'erreur et le résultat communs,
//! rien de plus. La lecture SMF (`midifile.rs`) est propre à l'import et vit
//! donc dans `scoretk-midi-import`. Aucun accès disque, aucun thread, aucune
//! horloge : la crate compile pour `wasm32-unknown-unknown`.

pub mod error;

pub use error::{Error, Result};
