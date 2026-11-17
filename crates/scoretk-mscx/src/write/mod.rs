// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/rw/write/` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Écriture `.mscx`.
//!
//! Le découpage suit `engraving/rw/write/` : [`writer`] (point d'entrée,
//! `Writer::writeScore`), [`staffwrite`] (parties, portées, cadres),
//! [`measurewrite`] (mesures et voix), [`twrite`] (éléments et annotations),
//! [`connectorinfowriter`] (spanners) et [`context`] (`WriteContext`).

pub mod connectorinfowriter;
pub mod context;
pub mod measurewrite;
pub mod staffwrite;
pub mod twrite;
pub mod writer;
