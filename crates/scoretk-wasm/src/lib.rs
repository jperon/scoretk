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

//! Bindings WebAssembly de la façade `scoretk`.
//!
//! Documentation : `docs/agents/wasm.md`.
//!
//! `nix build .#wasm` produit deux variantes à partir de ce crate :
//!
//! - `esm/` : module ES (`wasm-bindgen --target web`), initialisé par
//!   `await init()` ;
//! - `classic/` : script classique (`wasm-bindgen --target no-modules`)
//!   chargeable par `importScripts()`, accompagné du wrapper `classic/scoretk.js`
//!   qui expose `self.scoretk`.
//!
//! Aucune entrée/sortie fichier, aucune horloge : tout se passe en mémoire.
//! Ce crate ne contient aucune logique de conversion, seulement l'enveloppe de
//! la façade `scoretk`. Il n'a pas de test unitaire : ses fonctions n'ont de
//! sens qu'avec un runtime JavaScript (voir `tests/node-smoke.mjs`).

mod api;
#[cfg(feature = "engrave")]
mod engrave;
mod formats;
mod options;
mod timemap_js;

use wasm_bindgen::prelude::*;

/// Version de la façade, telle que déclarée dans `Cargo.toml`.
///
/// `skip_typescript` : la déclaration exacte est dans la section TypeScript de
/// [`api`], avec les autres fonctions exposées.
#[wasm_bindgen(skip_typescript)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
