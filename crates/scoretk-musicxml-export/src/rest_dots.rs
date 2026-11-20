// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage du comportement de `Rest::checkDots` et `writeRest` de MuseScore
// Studio, Copyright (C) MuseScore Limited and others, modifié (réécrit en
// Rust) en 2026.
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

//! Points d'un silence dans l'export MusicXML.
//!
//! `Rest::checkDots` (`engraving/dom/rest.cpp:308-321`) crée les objets
//! `NoteDot` en fonction de `dots()` ; il n'est appelé que par le layout de la
//! portée (`engraving/rendering/score/restlayout.cpp:148`, lui-même court-circuité
//! pour une portée invisible). `writeRest`
//! (`importexport/musicxml/internal/export/exportmusicxml.cpp:4655-4669`) écrit
//! ensuite `<dot>` **uniquement depuis `rest->dotList()`**. Le portage déduisait
//! les points de la figure `TDuration`, ce qui en ajoutait sur 155 silences du
//! lot (toutes sur des portées masquées, par exemple dans `Minuit chrétien`).

/// Nombre de `<dot/>` à écrire pour un silence.
pub(crate) fn dot_count(duration_dots: u8, staff_visible: bool) -> usize {
    if staff_visible {
        usize::from(duration_dots)
    } else {
        0
    }
}
