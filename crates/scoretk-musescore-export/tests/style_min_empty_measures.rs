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

//! `Sid::minEmptyMeasures` est un style du `.mscx` : sa valeur 3.x (2) doit
//! survivre à l'écriture 4.70, sinon l'inventaire des couches perd l'item
//! `style-internal` et la déduction des multi-repos change.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

fn corpus(name: &str) -> Vec<u8> {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    fs::read(racine.join(name)).expect("corpus")
}

#[test]
fn minimum_de_mesures_vides_3x_ecrit_et_relu() {
    let source = corpus("v3/barline01.mscx");
    let partition = lire(&source);
    assert_eq!(partition.engraving.style.min_empty_measures, 2);

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<minEmptyMeasures>2</minEmptyMeasures>"),
        "la valeur 3.x doit être réécrite dans <Style>"
    );
    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.engraving.style.min_empty_measures, 2);

    // Cas non concerné : le défaut 4.x n'ajoute pas le tag.
    let nu = lire(&corpus("v4/mmrest.mscx"));
    assert_eq!(nu.engraving.style.min_empty_measures, 1);
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu).expect("écriture");
    assert!(!ecrit_nu.contains("<minEmptyMeasures>"));
}
