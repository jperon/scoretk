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

//! Champs `\header` standards utilisés comme transport des crédits.
//!
//! La liste des champs par défaut de LilyPond est donnée par la Notation
//! Reference §21.1.2. Aucun de ces champs n'est privé : un autre outil les lit
//! et les affiche. L'ordre est partagé par l'écrivain et le lecteur, qui
//! reconstruisent ainsi l'index des `<credit>` du modèle sans directive
//! `%%scoretk`.

/// Champs `\header` standards disponibles pour des crédits supplémentaires,
/// dans l'ordre de reconstruction des index.
// `opus` est exclu : c'est le champ standard du numéro d'œuvre (§21.1.2),
// lu comme `work_number` par le lecteur. `breakbefore` accepte une valeur
// textuelle et compile sans avertissement avec LilyPond 2.26.
pub const CREDIT_HEADER_FIELDS: &[&str] = &[
    "subsubtitle",
    "instrument",
    "piece",
    "dedication",
    "tagline",
    "meter",
    "breakbefore",
];
