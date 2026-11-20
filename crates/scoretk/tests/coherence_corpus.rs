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

//! Invariant de cohérence des mesures sur le corpus `.ly` de l'écrivain.
//!
//! Le helper partagé vérifie que, dans chaque mesure, les durées des accords
//! et silences de chaque voix totalisent `Measure::len` (c'est l'invariant que
//! MuseScore 4.7.4 contrôle en rouvrant un `.mscz`, voir
//! `docs/agents/mscx-write.md` § « Second pass »). Les cas réduits avec
//! `\partial` multiplié sont dans `partial_multiplier.rs`.

#![cfg(feature = "lilypond-import")]

mod common;

use std::fs;
use std::path::Path;

use scoretk::{read, Format};

use common::consistency::assert_mesures_coherentes;

/// `v3_moonlight` a des voix qui s'arrêtent avant la fin de leur mesure
/// (paroles et voix internes incomplètes). C'est un écart de mise en page
/// déjà documenté dans `docs/agents/ecarts-noyau.md` et
/// `docs/agents/mscx-write.md` §7, sans rapport avec la lecture du `\partial` :
/// l'invariant « chaque voix couvre toute la mesure » ne s'y applique pas.
const VOIX_INCOMPLETES: &[&str] = &["v3_moonlight.ly"];

#[test]
fn coherence_des_mesures_du_corpus_ly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/ly");
    let mut lus = 0usize;
    for entry in fs::read_dir(&dir).expect("lecture du répertoire du corpus") {
        let path = entry.expect("entrée du corpus").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("ly") {
            continue;
        }
        let nom = path.file_name().expect("nom de fichier").to_string_lossy();
        let bytes = fs::read(&path).expect("contenu du `.ly`");
        let score =
            read(&bytes, Format::Ly).unwrap_or_else(|error| panic!("lecture de {nom} : {error}"));
        if !VOIX_INCOMPLETES.contains(&nom.as_ref()) {
            assert_mesures_coherentes(&score, &nom);
        }
        lus += 1;
    }
    assert!(lus > 0, "aucun `.ly` lu dans {}", dir.display());
}
