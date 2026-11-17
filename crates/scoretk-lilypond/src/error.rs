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

//! Erreurs de l'écriture LilyPond.

/// Erreur de l'écriture LilyPond.
///
/// L'écriture ne peut échouer que sur une partition inexploitable : toute
/// valeur douteuse du modèle (durée nulle, piste hors portée) est remplacée
/// par une valeur de repli plutôt que de faire échouer la conversion, pour
/// qu'aucun fichier du corpus ne soit perdu.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// La partition ne contient aucune portée : il n'y a aucune voix à écrire.
    #[error("partition sans portée : rien à écrire en LilyPond")]
    PartitionVide,
    /// La source LilyPond ne contient pas de musique lisible.
    #[error("source LilyPond illisible : {0}")]
    Syntaxe(String),
}

/// Résultat de l'écriture LilyPond.
pub type Result<T> = std::result::Result<T, Error>;
