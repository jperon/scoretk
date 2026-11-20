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

//! Propriété `<irregular>` d'une mesure d'anacrouse.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `MeasureWrite::writeMeasure` (`write/measurewrite.cpp:76`) écrit
/// `Pid::EXCLUDE_FROM_NUMBERING`, c'est-à-dire `<irregular>` (`property.cpp`,
/// table des propriétés). Le lecteur 4.6+ (`read460/measureread.cpp:124-125`)
/// le relit dans `Measure::excludeFromNumbering()` ; sans lui, MuseScore
/// réexporte la première mesure avec `number="1"` au lieu de
/// `number="0" implicit="yes"`.
#[test]
fn mesure_irreguliere_ecrit_sa_propriete() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("voices.mscx")).expect("corpus");
    let mut partition = lire(&source);
    assert!(!partition.measures.is_empty());
    let notes = partition.notes.len();
    let mesures = partition.measures.len();
    // Musique ordinaire conservée : voix, accords, silences et barres.
    partition.measures[0].exclude_from_numbering = true;

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<irregular>1</irregular>"),
        "une mesure exclue de la numérotation doit écrire <irregular>"
    );
    let relu = lire(ecrit.as_bytes());
    assert!(
        relu.measures[0].exclude_from_numbering,
        "la propriété doit survivre à la relecture"
    );
    let direct = scoretk_musicxml_export::export_musicxml(&partition).expect("export");
    let retour = scoretk_musicxml_export::export_musicxml(&relu).expect("export");
    assert_eq!(direct.matches("<pitch>").count(), retour.matches("<pitch>").count());
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

    // Cas non concerné : une mesure ordinaire n'invente pas la propriété.
    partition.measures[0].exclude_from_numbering = false;
    let ordinaire = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(!ordinaire.contains("<irregular>"));
}
