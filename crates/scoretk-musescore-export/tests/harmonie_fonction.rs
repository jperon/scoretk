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

//! Racine d'un accord chiffré de Nashville écrit depuis `<function>`.

use std::fs;
use std::path::Path;

use scoretk_core::model::{tpc, Annotation, Score};
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

fn harmonies(score: &Score) -> Vec<&scoretk_core::model::Harmony> {
    score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .flat_map(|segment| &segment.annotations)
        .filter_map(|entry| match &entry.annotation {
            Annotation::Harmony(harmony) => Some(harmony),
            _ => None,
        })
        .collect()
}

fn racines_dans(mscx: &str) -> usize {
    mscx.matches("<harmonyInfo>")
        .zip(mscx.match_indices("<harmonyInfo>").map(|(i, _)| {
            let fin = mscx[i..].find("</harmonyInfo>").unwrap_or(mscx.len() - i);
            mscx[i..i + fin].contains("<root>")
        }))
        .filter(|(_, has_root)| *has_root)
        .count()
}

/// `TWrite::write(const Harmony*)` (`write/twrite.cpp:1834-1868`) délègue à
/// `writeHarmonyInfo` (`:1786-1826`), qui écrit `<root>` depuis `rootTpc`. Le
/// lecteur 4.6+ (`read460/tread.cpp:2911`) ignore le `<function>` direct des
/// fichiers 3.x ; MuseScore le convertit en `rootTpc` à la lecture
/// (`read410/tread.cpp:2839`, `dom/pitchspelling.cpp:732` `function2Tpc`).
/// Faute de `<root>` dans `<harmonyInfo>`, MuseScore 4.7.4 réexporte un
/// `<function>` obsolète au lieu du `<numeral>` attendu.
#[test]
fn fonction_nashville_ecrit_une_racine() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("testHarmony6.mscx")).expect("corpus");
    let partition = lire(&source);
    let accords = harmonies(&partition);
    let avec_fonction: Vec<_> = accords
        .iter()
        .filter(|harmony| harmony.function.is_some() && harmony.root_tpc == tpc::INVALID)
        .collect();
    assert!(
        !avec_fonction.is_empty(),
        "le corpus doit porter un accord de Nashville sans racine"
    );
    let mesures = partition.measures.len();
    let notes = partition.notes.len();

    let racines_initiales = accords
        .iter()
        .filter(|harmony| harmony.root_tpc != tpc::INVALID)
        .count();
    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert_eq!(
        racines_dans(&ecrit),
        racines_initiales + avec_fonction.len(),
        "seuls les accords à <function> doivent gagner une racine"
    );
    // Musique ordinaire : hauteurs, durées, mesures et accords inchangés.
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
    assert_eq!(partition.measures.len(), mesures);
    assert_eq!(partition.notes.len(), notes);
    assert_eq!(harmonies(&relu).len(), accords.len());

    // Cas non concerné : l'accord romain du même corpus n'a ni fonction ni
    // racine ; son <harmonyInfo> ne doit pas en inventer une.
    let romain = accords
        .iter()
        .find(|harmony| harmony.function.is_none() && harmony.root_tpc == tpc::INVALID)
        .expect("accord romain sans racine ni fonction");
    let bloc_romain = ecrit
        .split("<harmonyInfo>")
        .skip(1)
        .map(|bloc| bloc.split("</harmonyInfo>").next().unwrap_or(""))
        .find(|bloc| bloc.contains(&format!("<name>{}</name>", romain.name)))
        .expect("bloc de l'accord romain");
    assert!(
        !bloc_romain.contains("<root>"),
        "un accord sans fonction ni racine ne doit pas gagner de <root>"
    );
}
