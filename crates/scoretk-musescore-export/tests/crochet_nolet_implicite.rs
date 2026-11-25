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

//! Crochet de n-olet implicite : ne pas écrire de propriété explicite.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) n'écrit
/// `Pid::BRACKET_TYPE` que si la propriété est explicitée ; sinon
/// `Tuplet::calcHasBracket()` (`:204-208`) la recalcule à la lecture. scoretk
/// écrivait `<bracket>`/`<bracketType>` même pour un crochet implicite, donc
/// MuseScore n'imposait plus son propre calcul et ses 12 triolets de mscz-18
/// réexportaient `bracket="no"` au lieu de `bracket="yes"`.
#[test]
fn crochet_implicite_pas_de_propriete() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("testTuplets8.mscx")).expect("corpus");
    let partition = lire(&source);
    assert!(partition.tuplets.iter().any(|t| !t.bracket_explicit));
    let notes = partition.notes.len();
    let mesures = partition.measures.len();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        !ecrit.contains("<bracketType>"),
        "aucun type de crochet ne doit être écrit pour un n-olet implicite"
    );
    assert!(!ecrit.contains("<bracket>"));

    // Cas non concerné : une propriété explicitée est bien réécrite.
    let mut explicite = partition.clone();
    explicite.tuplets[0].bracket_explicit = true;
    explicite.tuplets[0].bracket = true;
    let ecrit_explicite = scoretk_musescore_export::write_mscx(&explicite).expect("écriture");
    assert!(
        ecrit_explicite.contains("<bracketType>1</bracketType>"),
        "un crochet explicité doit rester SHOW_BRACKET"
    );
    let relu = lire(ecrit_explicite.as_bytes());
    assert!(relu.tuplets[0].bracket_explicit);
    assert!(relu.tuplets[0].bracket);
    let direct = scoretk_musicxml_export::export_musicxml(&explicite).expect("export");
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
}

/// Un triolet de croches lu depuis un fichier 3.x porte une décision de
/// crochet calculée par la règle 3.x (`Tuplet::calcHasBracket` : le n-olet est
/// groupé par sa ligature). Réécrit en 4.70, le lecteur 4.x recalcule
/// autrement ; sans propriété, 262 n-olets du corpus changeaient de crochet.
/// `TWrite::write(const Tuplet*)` (`engraving/rw/write/twrite.cpp:3321-3345`,
/// v4.7.4) écrit `Pid::BRACKET_TYPE` via `writeStyledProperties` : on ne
/// l'explicite que pour les n-olets courts dont la décision serait perdue.
#[test]
fn crochet_court_implicite_preserve_au_aller_retour() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("moonlight.mscx")).expect("corpus");
    let partition = lire(&source);
    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<bracketType>2</bracketType>"),
        "le crochet négatif décidé par la lecture 3.x doit être explicite"
    );
    let relu = lire(ecrit.as_bytes());
    assert_eq!(partition.tuplets.len(), relu.tuplets.len());
    for (avant, apres) in partition.tuplets.iter().zip(&relu.tuplets) {
        assert_eq!(
            avant.bracket, apres.bracket,
            "crochet modifié au tick {}",
            avant.tick
        );
    }
}
