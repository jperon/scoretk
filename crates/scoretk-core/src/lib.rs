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

//! Cœur générique de scoretk : modèle de partition et adressage d'éléments.
//!
//! Cette bibliothèque est volontairement sans entrées/sorties et sans aucun
//! format : elle travaille uniquement sur des structures en mémoire, sans
//! système de fichiers, sans thread et sans dépendance C. Elle doit donc
//! compiler pour `wasm32-unknown-unknown`, Android et iOS.
//!
//! Le module [`model`] contient le modèle de partition en mémoire, rempli par
//! les importeurs et parcouru par les exporteurs. [`duration_list`] décompose
//! une durée en figures, [`gap_rests`] comble les
//! trous de voix ; [`model::stable_ids`] calcule les identifiants d'éléments
//! partagés par les exporteurs.

pub mod duration_list;
mod error;
pub mod gap_rests;
pub mod model;

pub use error::{Error, Result};
