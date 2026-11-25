// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les correspondances de notation (articulations, ornements, nuances, clés,
// armures) s'inspirent de `musicxml2ly` de LilyPond, Copyright (C) 2016--2026
// John Gourlay et Werner Lemberg, GPL-3.0-or-later ; le code ci-dessous est
// une réécriture en Rust, sans ligne copiée.
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

//! **scoretk-lilypond-common** : le peu que partagent la lecture et l'écriture
//! LilyPond.
//!
//! Cette crate ne contient aucune logique de conversion : ni lecture, ni
//! écriture d'un fichier, seulement des tables et des types communs aux deux
//! sens (règle de projet, cf. `docs/agents/developpement.md`).
//!
//! - [`error`] : l'erreur unique des deux sens ([`Error`], [`Result`]) ;
//! - [`pitch`] : la spirale des quintes (`tpc`) et ses conversions ;
//! - [`clefs`] : la table des clefs, dans les deux sens.
//!
//! Tout le reste appartient à `scoretk-lilypond-import` (lecture `.ly` →
//! modèle) ou à `scoretk-lilypond-export` (modèle → `.ly`).
//!
//! Comme les deux autres, cette crate ne fait aucune entrée/sortie et compile
//! pour `wasm32-unknown-unknown`, Android et iOS.

pub mod clefs;
mod error;
pub mod noteheads;
pub mod pitch;

pub use error::{Error, Result};
