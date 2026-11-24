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

//! Style `createMultiMeasureRests` conservé dans le `.mscx` écrit.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `Sid::createMultiMeasureRests` (`style/styledef.cpp:571`, défaut `false`)
/// commande à `Measure::coveringMMRestOrThis()` (`dom/measure.cpp:2899`) de
/// reconstruire les multi-repos, et donc à l'export MusicXML d'écrire
/// `<multiple-rest>` (`importmusicxml.cpp:6587-6597`). Le lecteur 4.x lit le
/// style (`read460/readstyle.cpp`) et l'écrivain le réécrit dans `<Style>`.
#[test]
fn style_multi_repos_lu_et_ecrit() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("mmrest.mscx")).expect("corpus");
    let partition = lire(&source);
    assert!(
        partition.engraving.style.create_multi_measure_rests,
        "le corpus active les multi-repos"
    );
    let notes = partition.notes.len();
    let mesures = partition.measures.len();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<createMultiMeasureRests>1</createMultiMeasureRests>"),
        "le style doit être réécrit dans <Style>"
    );
    let relu = lire(ecrit.as_bytes());
    assert!(
        relu.engraving.style.create_multi_measure_rests,
        "le style doit survivre à la relecture"
    );
    let direct = scoretk_musicxml_export::export_musicxml(&partition).expect("export");
    let retour = scoretk_musicxml_export::export_musicxml(&relu).expect("export");
    assert_eq!(
        direct.matches("<pitch>").count(),
        retour.matches("<pitch>").count()
    );
    assert_eq!(
        direct.matches("<duration>").count(),
        retour.matches("<duration>").count()
    );
    assert_eq!(
        direct.matches("<measure ").count(),
        retour.matches("<measure ").count()
    );
    assert_eq!(partition.measures.len(), mesures);
    assert_eq!(partition.notes.len(), notes);

    // Cas non concerné : une partition sans ce style n'invente pas le tag.
    let nu = fs::read(racine.join("testNoteheads2.mscx")).expect("corpus");
    let nu_score = lire(&nu);
    assert!(!nu_score.engraving.style.create_multi_measure_rests);
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu_score).expect("écriture");
    assert!(!ecrit_nu.contains("<createMultiMeasureRests>"));
}
