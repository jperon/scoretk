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

//! Écriture des deux types de clef pour le lecteur 4.6+ de MuseScore.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const Clef*)` (`write/twrite.cpp:1281-1299`) écrit
/// `Pid::CLEF_TYPE_CONCERT` **et** `Pid::CLEF_TYPE_TRANSPOSING`. Le lecteur
/// (`read400/tread.cpp:2551-2553`) force la clé en G si le type transposant
/// manque : sans `<transposingClefType>`, MuseScore 4.7.4 rouvre une portée
/// de basse en clé de sol, et l'export MusicXML change de `<sign>`.
#[test]
fn clef_basse_ecrit_son_type_transposant() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("clef-1.mscx")).expect("corpus");
    let partition = lire(&source);

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    let concert = ecrit.matches("<concertClefType>F</concertClefType>").count();
    let transposant = ecrit
        .matches("<transposingClefType>F</transposingClefType>")
        .count();
    assert!(concert > 0, "le corpus doit porter des clefs de basse");
    assert_eq!(
        concert, transposant,
        "chaque clef concert F doit porter le même type transposant"
    );

    // Le reste de la mesure est inchangé : hauteurs, durées et mesures.
    let relu = lire(ecrit.as_bytes());
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
}
