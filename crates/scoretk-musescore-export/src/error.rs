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

//! Erreur publique de l'écrivain `.mscx` / `.mscz`.

/// Erreurs de l'écriture MuseScore.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// L'archive ZIP (`.mscz`) n'a pas pu être produite.
    #[error("archive ZIP illisible : {0}")]
    Zip(#[from] zip::result::ZipError),
}
