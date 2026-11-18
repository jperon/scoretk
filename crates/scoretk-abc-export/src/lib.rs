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

//! **scoretk-abc-export** : écriture ABC depuis le modèle pivot.
//!
//! Le crate ne fait aucune entrée/sortie : [`write_abc`] travaille en mémoire,
//! ce qui le garde compilable pour `wasm32-unknown-unknown`, Android et iOS.
//! Les types et helpers partagés avec la lecture vivent dans
//! `scoretk-abc-common`.
//!
//! [`write_abc`] produit un document ABC 2.1 complet : en-tête `X:`, `T:`,
//! `C:`, `M:`, `L:`, `Q:`, `K:` (avec clef), voix `V:` multiples, durées
//! relatives à `L:1/8`, n-olets `(3`, liaisons `-`, phrasés `( )`, accords
//! `[ ]`, appoggiatures `{ }`, barres et reprises `|: :| [1 [2`, nuances
//! `!p!`, articulations `!staccato!`, paroles `w:`, annotations `"^texte"` et
//! accords chiffrés `"C"`. La correspondance complète et les limites sont
//! décrites dans `docs/agents/abc.md`.
//!
//! # Exemple
//!
//! ```ignore
//! let score = scoretk_abc_import::read_abc("X:1\nT:Air\nM:4/4\nL:1/8\nK:C\nC D E F |]\n")
//!     .expect("lecture");
//! let abc = scoretk_abc_export::write_abc(&score).expect("écriture");
//! assert!(abc.starts_with("%abc-2.1\nX:1\n"));
//! assert!(abc.contains("K:C"));
//! ```
//!
//! L'exemple suppose `scoretk-abc-import` en dev-dépendance ; l'écriture seule
//! n'en a pas besoin. Voir `docs/agents/abc.md`.

mod voice;
mod writer;

use scoretk_core::model::Score;

pub use scoretk_abc_common::{Error, Result, Warning};

/// Écrit une partition du modèle pivot au format ABC.
///
/// La sortie est un document complet, reproductible (aucune horloge, aucun
/// identifiant aléatoire), terminé par un saut de ligne.
pub fn write_abc(score: &Score) -> Result<String> {
    writer::Writer::new(score).render()
}
