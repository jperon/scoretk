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

//! Avertissements de la lecture LilyPond.
//!
//! Le sous-ensemble lu ignore tout ce qu'il ne comprend pas (Scheme
//! arbitraire, commandes inconnues, propriétés non modélisées) : chaque
//! abandon produit un [`Warning`] daté plutôt qu'une erreur, comme le fait
//! `python-ly` (`ly.music`) pour un lecteur sans Scheme.

use std::fmt;

/// Avertissement de lecture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// Ligne de la source, à partir de 1 ; 0 quand la position est inconnue.
    pub line: u32,
    /// Colonne, à partir de 1 ; 0 quand la position est inconnue.
    pub column: u32,
    /// Message.
    pub message: String,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}:{} : {}", self.line, self.column, self.message)
        }
    }
}

/// Collecteur d'avertissements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Warnings {
    items: Vec<Warning>,
    /// Un `\include` ignoré pouvait porter la partition : l'absence de
    /// `\score` n'est alors pas une erreur de syntaxe, mais une limite du
    /// sous-ensemble lu.
    music_ignoree: bool,
}

impl Warnings {
    /// Collecteur vide.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Ajoute un avertissement.
    pub(crate) fn push(&mut self, line: u32, column: u32, message: impl Into<String>) {
        self.items.push(Warning {
            line,
            column,
            message: message.into(),
        });
    }

    /// Avertissements collectés.
    pub(crate) fn into_vec(self) -> Vec<Warning> {
        self.items
    }

    /// Note qu'un include ignoré pouvait contenir la musique.
    pub(crate) fn mark_music_ignored(&mut self) {
        self.music_ignoree = true;
    }

    /// Vrai si un include ignoré pouvait contenir la musique.
    pub(crate) fn music_ignored(&self) -> bool {
        self.music_ignoree
    }
}
