// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier ne porte pas de code C++ : il ajoute au modèle pivot la
// transposition de lecture trouvée dans une source LilyPond dont le MIDI est
// produit par un second `\score` transposé. Rédigé en Rust en 2026 d'après la
// Notation Reference de LilyPond 2.26 (§1.2.2 `\transpose`, §24.3 « The MIDI
// block ») : aucun C++ amont n'est distribué.
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

//! Transposition de lecture du modèle pivot.
//!
//! Ce champ est **ignoré par tous les écrivains sauf MIDI** : MusicXML,
//! LilyPond, ABC et MuseScore écrivent la partition gravée telle quelle. Le
//! lecteur LilyPond le remplit quand la source contient un second `\score`
//! MIDI (`\midi` sans `\layout`) dont la musique est la même que la partition
//! gravée, enveloppée dans `\transpose de vers` (Notation Reference §1.2.2,
//! §24.3).

/// Décalage chromatique à appliquer aux seules hauteurs jouées (MIDI).
///
/// L'intervalle est exprimé en demi-tons : `\transpose c d` vaut `2`,
/// `\transpose d c` vaut `-2`, `\transpose c c'` vaut `12`. Il s'ajoute aux
/// `Note::pitch` du modèle au moment du rendu MIDI ; il ne modifie ni les
/// hauteurs écrites, ni les armures du modèle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaybackTransposition {
    /// Décalage chromatique, en demi-tons, appliqué aux notes MIDI.
    pub chromatic: i32,
}

impl PlaybackTransposition {
    /// Construit une transposition de lecture à partir d'un écart en demi-tons.
    pub const fn new(chromatic: i32) -> Self {
        Self { chromatic }
    }
}
