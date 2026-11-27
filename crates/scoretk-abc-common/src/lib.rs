// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// MuseScore n'a pas d'import ni d'export ABC natif : ce crate suit la norme
// ABC 2.1 (<https://abcnotation.com/wiki/abc:standard:v2.1>) et le
// comportement de `xml2abc.py` et `abc2xml.py` de Willem Vree, distribués
// sous LGPL (sans numéro de version dans leur en-tête ; la LGPL 2.1 § 3
// autorise la conversion en GPL). Aucune ligne de ces scripts n'est copiée :
// l'implémentation est une réécriture Rust.
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

//! **scoretk-abc-common** : parties ABC partagées par la lecture et l'écriture.
//!
//! Ce crate ne fait aucune entrée/sortie et ne dépend ni de
//! [`scoretk-abc-import`](../scoretk_abc_import/index.html) ni de
//! [`scoretk-abc-export`](../scoretk_abc_export/index.html). Il ne contient que
//! les éléments que les **deux sens** emploient réellement :
//!
//! - [`error`] : [`Error`], [`Warning`] et [`Result`] ;
//! - [`pitch`] : tables de hauteurs, altérations, armures, clés et modes,
//!   dont [`pitch::AccidentalState`] (état des altérations d'une mesure) ;
//! - [`duration`] : multiplicateurs relatifs à `L:` et figures du modèle ;
//! - [`decorations`] : liste des nuances, articulations et ornements ;
//! - [`text`] : échappement des annotations et des paroles `w:` ;
//! - [`gregorian`] : porteurs ABC 2.1 partagés par le chant grégorien.

pub mod decorations;
pub mod duration;
pub mod error;
pub mod gregorian;
pub mod pitch;
pub mod text;

pub use error::{Error, Result, Warning};
