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

//! Style `concertPitch` conservé dans le `.mscx` écrit.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `Sid::concertPitch` (`style/styledef.cpp:567`, défaut `false`) est écrit
/// dans `<Style>` par `MscWriter::writeStyleFile`
/// (`infrastructure/mscwriter.cpp:135`) et lu par `TRead::read(Score*)`. Le
/// lecteur 4.x de scoretk le lisait (`StyleSettings::set_value`) mais
/// l'écrivain ne le réécrivait pas : MuseScore réouvrait la partition en
/// hauteurs transposées (121 → 89 lignes de noyau sur mscz-165).
#[test]
fn style_concert_pitch_lu_et_ecrit() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("voices.mscx")).expect("corpus");
    let mut partition = lire(&source);
    assert!(!partition.style.concert_pitch);
    partition.style.concert_pitch = true;
    let notes = partition.notes.len();
    let mesures = partition.measures.len();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<concertPitch>1</concertPitch>"),
        "le style doit être réécrit dans <Style>"
    );
    let relu = lire(ecrit.as_bytes());
    assert!(
        relu.style.concert_pitch,
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

    // Cas non concerné : sans le style, aucun tag n'est inventé.
    partition.style.concert_pitch = false;
    let ordinaire = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(!ordinaire.contains("<concertPitch>"));
}
