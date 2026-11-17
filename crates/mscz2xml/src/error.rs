// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de scoretk
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

//! Erreur publique de la bibliothèque.

use crate::container::MscVersion;

/// Toutes les erreurs pouvant survenir pendant la lecture d'un conteneur.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Le conteneur ZIP est illisible ou corrompu.
    #[error("conteneur ZIP invalide : {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Le XML est mal formé.
    #[error("XML invalide : {0}")]
    Xml(#[from] quick_xml::Error),

    /// Un attribut XML est mal formé.
    #[error("attribut XML invalide : {0}")]
    Attribute(#[from] quick_xml::events::attributes::AttrError),

    /// `META-INF/container.xml` existe mais ne désigne aucun fichier racine.
    #[error("META-INF/container.xml ne désigne aucun fichier racine")]
    NoRootFile,

    /// Aucun fichier `.mscx` n'a été trouvé dans le conteneur.
    #[error("aucun fichier .mscx trouvé dans le conteneur")]
    NoMscxFound,

    /// Le fichier racine annoncé par `container.xml` est absent du ZIP.
    #[error("fichier racine « {0} » introuvable dans le conteneur")]
    RootFileNotFound(String),

    /// L'élément `<museScore>` n'a pas d'attribut `version`.
    #[error("attribut « version » absent de l'élément <museScore>")]
    MissingVersion,

    /// L'attribut `version` n'est pas un nombre de version reconnu.
    #[error("version MuseScore invalide : « {0} »")]
    InvalidVersion(String),

    /// La version MuseScore du fichier n'est pas prise en charge par un
    /// lecteur de cette bibliothèque.
    #[error("version MuseScore non prise en charge : {0}")]
    UnsupportedVersion(MscVersion),

    /// Le fichier ne contient aucun élément `<Score>` exploitable.
    #[error("aucun élément <Score> dans le fichier .mscx")]
    MissingScore,
}
