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

//! Indice de grâce d'une extrémité de liaison de prolongation.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

fn ties(score: &Score) -> usize {
    score
        .notes
        .iter()
        .filter(|note| note.tie_forward.is_some() || note.tie_back.is_some())
        .count()
}

/// `TWrite::write(const Location*)` (`write/twrite.cpp:2355-2368`) écrit
/// `Pid::LOCATION_GRACE` (`<grace>`), que `ConnectorInfoReader` utilise pour
/// retrouver l'accord d'agrément anonyme d'une liaison
/// (`read460/connectorinforeader.cpp:346-360`, `dom/location.cpp:196-207`).
/// Sans lui, MuseScore 4.7.4 ne réexporte pas les quatre `<tie>` du corpus.
#[test]
fn liaison_depuis_note_dagrement_ecrit_grace() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("linked-ties.mscx")).expect("corpus");
    let partition = lire(&source);
    assert!(ties(&partition) > 0, "le corpus doit porter des liaisons");
    let notes = partition.notes.len();
    let mesures = partition.measures.len();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<grace>0</grace>"),
        "une liaison depuis une note d'agrément doit écrire son indice de grâce"
    );
    let relu = lire(ecrit.as_bytes());
    assert_eq!(ties(&relu), ties(&partition));
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

    // Cas non concerné : des liaisons ordinaires n'inventent pas d'indice.
    let ordinaires = fs::read(racine.join("copypaste_parts.mscx")).expect("corpus");
    let score_ordinaire = lire(&ordinaires);
    assert!(ties(&score_ordinaire) > 0);
    let ecrit_ordinaire = scoretk_musescore_export::write_mscx(&score_ordinaire).expect("écriture");
    assert!(!ecrit_ordinaire.contains("<grace>0</grace>"));
}
