// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original (signature d'erreur commune aux deux sens MEI).
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

//! Erreur commune aux deux directions MEI.

/// Erreur de lecture ou d'écriture MEI.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Le document n'est pas de l'UTF-8 valide.
    #[error("document MEI non UTF-8 : {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// L'arbre XML est illisible.
    #[error("MEI : XML invalide : {0}")]
    Xml(#[from] scoretk_xml_common::Error),

    /// Pas d'élément `<score>` exploitable (`MeiImporter::readScore`).
    #[error("aucun élément <score> dans le document MEI")]
    NoScore,

    /// Plus de quatre `<layer>` sur une portée : au-delà de `VOICES`.
    #[error("plus de quatre couches sur une portée MEI (limite du modèle pivot)")]
    TooManyLayers,

    /// Valeur non gérée par le port.
    #[error("MEI : {0}")]
    Unsupported(String),
}

/// Résultat des opérations MEI.
pub type Result<T> = std::result::Result<T, Error>;
