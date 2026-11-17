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

//! **scoretk-abc** : lecture et écriture ABC depuis le modèle pivot.
//!
//! Le crate ne fait aucune entrée/sortie : [`write_abc`] et [`read_abc`]
//! travaillent en mémoire, ce qui le garde compilable pour
//! `wasm32-unknown-unknown`, Android et iOS.
//!
//! # Écriture
//!
//! [`write_abc`] produit un document ABC 2.1 complet : en-tête `X:`, `T:`,
//! `C:`, `M:`, `L:`, `Q:`, `K:` (avec clef), voix `V:` multiples, durées
//! relatives à `L:1/8`, n-olets `(3`, liaisons `-`, phrasés `( )`, accords
//! `[ ]`, appoggiatures `{ }`, barres et reprises `|: :| [1 [2`, nuances
//! `!p!`, articulations `!staccato!`, paroles `w:`, annotations `"^texte"` et
//! accords chiffrés `"C"`. La correspondance complète et les limites sont
//! décrites dans `docs/agents/abc.md`.
//!
//! # Lecture
//!
//! [`read_abc`] lit le premier morceau d'un fichier (un fichier ABC peut en
//! contenir plusieurs, séparés par `X:`) ; [`read_abc_tune`] choisit le
//! morceau et [`read_abc_with`] renvoie aussi les avertissements. Le
//! sous-ensemble couvert est celui de l'écrivain, plus les modes de `K:`
//! (ramenés à leur armure), le rythme brisé et les ornements abrégés. Les
//! directives `%%` sont ignorées avec un avertissement.
//!
//! # Exemple
//!
//! ```
//! let score = scoretk_abc::read_abc("X:1\nT:Air\nM:4/4\nL:1/8\nK:C\nC D E F |]\n")
//!     .expect("lecture");
//! assert_eq!(score.measures.len(), 1);
//! let abc = scoretk_abc::write_abc(&score).expect("écriture");
//! assert!(abc.starts_with("%abc-2.1\nX:1\n"));
//! assert!(abc.contains("K:C"));
//! ```

mod decorations;
mod duration;
mod error;
mod pitch;
mod reader;
mod text;
mod voice;
mod writer;

pub use error::{Error, Result, Warning};
pub use reader::{read_abc, read_abc_tune, read_abc_with, ReadOptions, ReadOutput};

use scoretk_core::model::Score;

/// Écrit une partition du modèle pivot au format ABC.
///
/// La sortie est un document complet, reproductible (aucune horloge, aucun
/// identifiant aléatoire), terminé par un saut de ligne.
pub fn write_abc(score: &Score) -> Result<String> {
    writer::Writer::new(score).render()
}
