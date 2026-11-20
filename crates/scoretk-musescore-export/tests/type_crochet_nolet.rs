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

//! Type de crochet d'un n-olet (propriété de style 4.6+).

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) écrit les
/// propriétés de style, dont `Pid::BRACKET_TYPE` (`dom/property.cpp`) :
/// `TupletBracketType::SHOW_BRACKET = 1`, `SHOW_NO_BRACKET = 2`
/// (`types.h:1604-1607`). Le lecteur 4.6+ ignore `<bracket>` (balise 3.x) et
/// ne garde que `bracketType` pour `calcHasBracket` (`dom/tuplet.cpp:204-208`).
/// Sans lui, MuseScore recalcule le crochet et perd 21 `.mscz` du corpus.
#[test]
fn type_crochet_nolet_ecrit_et_relu() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("testTuplets8.mscx")).expect("corpus");
    let mut partition = lire(&source);
    assert!(partition.tuplets.len() >= 2, "le corpus porte des n-olets");
    partition.tuplets[0].bracket = true;
    partition.tuplets[1].bracket = false;
    let notes = partition.notes.len();
    let mesures = partition.measures.len();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<bracketType>1</bracketType>"),
        "un crochet affiché doit écrire SHOW_BRACKET"
    );
    assert!(
        ecrit.contains("<bracketType>2</bracketType>"),
        "un crochet masqué doit écrire SHOW_NO_BRACKET"
    );
    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.tuplets.len(), partition.tuplets.len());
    assert!(relu.tuplets[0].bracket);
    assert!(!relu.tuplets[1].bracket);
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

    // Cas non concerné : une musique sans n-olet n'invente pas de type.
    let nu = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("corpus")
            .join("v4")
            .join("testNoteheads2.mscx"),
    )
    .expect("corpus");
    let nu_score = lire(&nu);
    assert!(nu_score.tuplets.is_empty());
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu_score).expect("écriture");
    assert!(!ecrit_nu.contains("<bracketType>"));
}
