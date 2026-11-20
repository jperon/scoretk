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

//! Écriture du type de marqueur de reprise pour le lecteur 4.6+ de MuseScore.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const Marker*)` (`write/twrite.cpp:2401-2409`) écrit
/// `Pid::MARKER_TYPE` (`<markerType>`, table `typesconv.cpp:3254`) en plus du
/// `<label>`. Le lecteur 4.6+ (`read460/tread.cpp:1724-1727`) ne reconstruit le
/// type qu'à partir de `<markerType>` ; sans lui, `Marker::Marker`
/// (`dom/marker.cpp:79`) initialise le type à `FINE`, et MuseScore réexporte
/// `<sound fine="yes"/>` au lieu de `<sound segno="segno"/>`.
#[test]
fn marqueur_segno_ecrit_son_type() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("repeat69.mscx")).expect("corpus");
    let partition = lire(&source);
    assert!(
        partition.measures.iter().any(|m| m
            .repeat_marks
            .iter()
            .any(|mark| { matches!(mark, scoretk_core::model::RepeatMark::Marker(_)) })),
        "le corpus doit porter un marqueur de reprise"
    );

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<markerType>segno</markerType>"),
        "le marqueur segno doit écrire son type pour le lecteur 4.6+"
    );

    // Cas non concerné : une partition sans marqueur n'invente pas de type.
    let sans = fs::read(racine.join("testNoteheads2.mscx")).expect("corpus");
    let sans_marqueur = lire(&sans);
    let ecrit_sans = scoretk_musescore_export::write_mscx(&sans_marqueur).expect("écriture");
    assert!(
        !ecrit_sans.contains("<markerType>"),
        "aucun <markerType> ne doit être inventé sans marqueur"
    );
}
