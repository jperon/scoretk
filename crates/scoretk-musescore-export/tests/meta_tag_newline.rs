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

//! `<metaTag>` : `Score::setMetaTag` (`engraving/dom/score.cpp`) conserve le
//! texte tel quel, retours à la ligne compris. Le lecteur 4.x utilisait
//! `Element::text()` (qui retire les espaces de bord), donc un `workNumber`
//! multiligne perdait son dernier saut de ligne à l'aller-retour.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

#[test]
fn meta_tag_multiligne_survit_a_l_ecriture() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    let source = fs::read(racine.join("v3/testHarmony6.mscx")).expect("corpus");
    let original = lire(&source);
    assert!(original.metadata.work_number.contains('\n'));
    let ecrit = scoretk_musescore_export::write_mscx(&original).expect("écriture");
    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.metadata.work_number, original.metadata.work_number);
}
