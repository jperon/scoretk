// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiconverter.{cpp,h}` de MuseScore
// Studio 4.7.4, Copyright (C) MuseScore Limited and others, modifié (réécrit
// en Rust) en 2026.
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

//! Ce que la lecture et l'écriture MEI partagent : constante d'espace de
//! noms, valeurs `@type` propriétaires de MuseScore et conversions
//! élément/atribut ↔ modèle (`Convert` dans `meiconverter.cpp`).
//!
//! MEI 5.1 : <https://music-encoding.org>. Les valeurs `@type` préfixées par
//! `mscore-` sont des extensions de MuseScore, pas du MEI standard.

pub mod convert;
pub mod error;
pub mod gabc_layout;
pub mod ids;
pub mod nabc;
pub mod neumes;

pub use error::{Error, Result};
