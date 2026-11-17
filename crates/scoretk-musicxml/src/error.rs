// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/import/` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Erreur publique du lecteur MusicXML.

/// Toutes les erreurs pouvant survenir pendant la lecture d'un fichier
/// MusicXML.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Le XML est mal formé.
    #[error("XML invalide : {0}")]
    Xml(#[from] quick_xml::Error),

    /// Un attribut XML est mal formé.
    #[error("attribut XML invalide : {0}")]
    Attribute(#[from] quick_xml::events::attributes::AttrError),

    /// Le conteneur `.mxl` (ZIP) est illisible.
    #[error("conteneur .mxl invalide : {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Le document n'est pas de l'UTF-8 valide.
    #[error("document non UTF-8 : {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// La racine du document n'est ni `score-partwise` ni `score-timewise`.
    #[error("racine MusicXML inconnue : « {0} » (attendu score-partwise)")]
    UnknownRoot(String),

    /// `META-INF/container.xml` existe mais ne désigne aucun fichier racine.
    #[error("META-INF/container.xml ne désigne aucun fichier racine")]
    NoRootFile,

    /// Le fichier racine annoncé par `container.xml` est absent du ZIP.
    #[error("fichier racine « {0} » introuvable dans le conteneur .mxl")]
    RootFileNotFound(String),

    /// Aucun `<part-list>` exploitable.
    #[error("aucun élément <part-list> dans le document")]
    MissingPartList,

    /// Aucune partie dans le document.
    #[error("aucune partie dans le document MusicXML")]
    NoParts,
}

/// Résultat du lecteur MusicXML.
pub type Result<T> = std::result::Result<T, Error>;
