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

//! Couches que le format MuseScore `.mscx`/`.mscz` peut porter (§5.3).
//!
//! Source : `docs/agents/format-ref/musescore.md` § « Section index » — §4,
//! §5, §7, §10, §12, §13, §15, §18 — et
//! `docs/agents/format-mapping/musescore.md` lignes 14–37 : L0–L6. La
//! déclaration décrit le **format**, pas ce que l'écrivain sait aujourd'hui
//! produire (ex. les excerpts et les objets graphiques manquent encore).

use scoretk_core::model::{Capabilities, Layers};

/// Contrat de l'écrivain MuseScore : écrit toutes les couches du format.
pub const CAPABILITIES: Capabilities = Capabilities::new(Layers::NONE, Layers::ALL);
