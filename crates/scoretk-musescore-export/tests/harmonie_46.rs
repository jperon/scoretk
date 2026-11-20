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

//! Écriture des accords chiffrés pour le lecteur 4.6+ de MuseScore.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

fn exporter(score: &Score) -> String {
    scoretk_musicxml_export::export_musicxml(score).expect("export MusicXML")
}

/// `writeHarmonyInfo` (`engraving/rw/write/twrite.cpp:1786-1826`) place
/// `<name>`/`<root>` dans `<harmonyInfo>`. Depuis 4.6, le lecteur
/// (`read460/tread.cpp:2868-2879`) ignore ces balises directement sous
/// `<Harmony>` : un fichier 4.70 écrit à l'ancienne perd tous ses accords
/// chiffrés au réouverture par MuseScore 4.7.4.
#[test]
fn accords_chiffres_relus_par_musescore_46() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("testHarmony6.mscx")).expect("corpus");
    let partition = lire(&source);

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    let accords = ecrit.matches("<Harmony>").count();
    let infos = ecrit.matches("<harmonyInfo>").count();
    assert!(accords > 0, "le corpus doit porter des accords chiffrés");
    assert_eq!(
        infos, accords,
        "chaque <Harmony> doit porter son <harmonyInfo> lisible par MuseScore 4.7.4"
    );

    // Le lecteur de scoretk garde les accords, les hauteurs, les durées et les
    // mesures : les enfants directs sont toujours écrits à côté.
    let relu = lire(ecrit.as_bytes());
    let direct = exporter(&partition);
    let retour = exporter(&relu);
    assert_eq!(
        direct.matches("<harmony ").count(),
        retour.matches("<harmony ").count(),
        "les accords chiffrés doivent survivre à l'aller-retour"
    );
    assert_eq!(
        direct.matches("<pitch>").count(),
        retour.matches("<pitch>").count(),
        "les hauteurs doivent survivre à l'aller-retour"
    );
    assert_eq!(
        direct.matches("<duration>").count(),
        retour.matches("<duration>").count(),
        "les durées doivent survivre à l'aller-retour"
    );
    assert_eq!(
        direct.matches("<measure ").count(),
        retour.matches("<measure ").count(),
        "le nombre de mesures doit survivre à l'aller-retour"
    );
}
