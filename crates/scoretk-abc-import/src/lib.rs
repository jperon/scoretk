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

//! **scoretk-abc-import** : lecture ABC vers le modèle pivot.
//!
//! Le crate ne fait aucune entrée/sortie : [`read_abc`] travaille en mémoire,
//! ce qui le garde compilable pour `wasm32-unknown-unknown`, Android et iOS.
//! Les types et helpers partagés avec l'écriture vivent dans
//! `scoretk-abc-common`.
//!
//! [`read_abc`] lit le premier morceau d'un fichier (un fichier ABC peut en
//! contenir plusieurs, séparés par `X:`) ; [`read_abc_tune`] choisit le
//! morceau et [`read_abc_with`] renvoie aussi les avertissements. Le
//! sous-ensemble couvert est celui de l'écriture, plus les modes de `K:`
//! (ramenés à leur armure), le rythme brisé et les ornements abrégés. Les
//! directives `%%` sont ignorées avec un avertissement.
//!
//! # Exemple
//!
//! ```
//! let score = scoretk_abc_import::read_abc("X:1\nT:Air\nM:4/4\nL:1/8\nK:C\nC D E F |]\n")
//!     .expect("lecture");
//! assert_eq!(score.measures.len(), 1);
//! ```

pub mod capabilities;
mod gregorian;
mod reader;

pub use capabilities::CAPABILITIES;

pub use reader::{read_abc, read_abc_tune, read_abc_with, ReadOptions, ReadOutput};

/// Lit un document ABC portant le dialecte grégorien additif.
///
/// Sans le marqueur `I:chant=gregorian`, le résultat est celui de [`read_abc`].
pub fn read_abc_gregorian(src: &str) -> Result<Score> {
    reader::read_abc_with(src, &ReadOptions {
        gregorian: true,
        ..ReadOptions::default()
    })
    .map(|output| output.score)
}
pub use scoretk_abc_common::{Error, Result, Warning};
use scoretk_core::model::Score;
