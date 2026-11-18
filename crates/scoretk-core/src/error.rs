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

//! Erreur générique du modèle.
//!
//! Le cœur ne connaît aucun format : cette erreur ne sert qu'aux opérations
//! génériques du modèle et aux crates de conversion qui choisissent de la
//! réutiliser. Les erreurs propres à un lecteur ou à un écrivain restent dans
//! le crate concerné.

/// Erreur générique de `scoretk-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Données invalides pour l'opération demandée.
    #[error("{0}")]
    Message(String),
}

/// Résultat de `scoretk-core`.
pub type Result<T> = std::result::Result<T, Error>;
