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

//! Instrument par défaut d'une partition sans instrument renseigné.

use std::fs;
use std::path::Path;

use scoretk_core::model::{Instrument, Score};
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// Aucune norme écrite (`format-ref/musescore.md`) : le comportement de
/// MuseScore 4.7.4 fait foi. Son écrivain (`TWrite::write(const Instrument*)`,
/// `write/twrite.cpp:1947`) produit toujours un instrument valide ; un
/// `<Instrument/>` vide est refusé au chargement (code 1, sans message). Les
/// sources LilyPond/ABC n'ayant pas d'instrument dans le modèle pivot,
/// l'écrivain doit y substituer l'instrument par défaut de MuseScore.
#[test]
fn instrument_vide_ecrit_un_instrument_valide() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("dynamics.mscx")).expect("corpus");
    let mut partition = lire(&source);
    assert!(!partition.parts.is_empty(), "le corpus doit avoir une partie");
    partition.parts[0].instrument = Instrument::new("", "", "");

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        !ecrit.contains("<Instrument/>"),
        "MuseScore 4.7.4 refuse un <Instrument/> vide"
    );
    assert!(
        ecrit.contains("<Channel>"),
        "l'instrument de repli doit avoir un canal MIDI"
    );

    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.parts.len(), partition.parts.len(), "parties relues");

    // La musique, elle, est inchangée : hauteurs, durées et mesures.
    let direct = scoretk_musicxml_export::export_musicxml(&partition).expect("export");
    let retour = scoretk_musicxml_export::export_musicxml(&relu).expect("export");
    assert_eq!(direct.matches("<pitch>").count(), retour.matches("<pitch>").count());
    assert_eq!(direct.matches("<duration>").count(), retour.matches("<duration>").count());
    assert_eq!(direct.matches("<measure ").count(), retour.matches("<measure ").count());
}
