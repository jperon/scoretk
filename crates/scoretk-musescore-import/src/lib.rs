// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/rwregister.cpp` et des lecteurs `read302`/`read4xx`
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

//! Lecture des partitions MuseScore (`.mscz`/`.mscx`) vers le modèle.
//!
//! Cette crate est volontairement sans entrées/sorties : elle travaille sur
//! des octets en mémoire et compile pour `wasm32-unknown-unknown`.
//!
//! [`MsczContainer`] extrait le fichier `.mscx` principal d'un conteneur
//! `.mscz` (ou accepte un `.mscx` nu) et [`detect_version`] lit la version du
//! format. [`read::read_score`] répartit ensuite vers le lecteur 3.x ou 4.x.

mod container;
mod error;
pub mod read;

pub use container::{detect_version, MscVersion, MsczContainer};
pub use error::{Error, Result};
