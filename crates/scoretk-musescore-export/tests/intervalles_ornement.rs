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

//! Intervalles d'un ornement lus puis réécrits dans le `.mscx`.

use std::fs;
use std::path::Path;

use scoretk_core::model::{OrnamentType, Score, SegmentItem};
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

fn ornements(score: &Score) -> Vec<&scoretk_core::model::Ornament> {
    score
        .measures
        .iter()
        .flat_map(|measure| &measure.segments)
        .flat_map(|segment| segment.elements())
        .filter_map(|item| match item {
            Some(SegmentItem::ChordRest(chord_rest)) => chord_rest.chord_ref(),
            _ => None,
        })
        .flat_map(|chord| &chord.ornaments)
        .collect()
}

/// `TRead::readProperties(Ornament*)` (`read460/tread.cpp:1868-1872`) lit
/// `Pid::INTERVAL_ABOVE` et `Pid::INTERVAL_BELOW`. Sans ces deux propriétés,
/// MuseScore reconstruit un turn avec ses intervalles par défaut et perd
/// l'<accidental>natural</accidental> de la mesure suivante à la réouverture
/// (vérifié sur `changevisibility`, core 1 ligne).
#[test]
fn intervalles_dornement_lus_et_ecrits() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("changevisibility.mscx")).expect("corpus");
    let partition = lire(&source);
    let ornements_initiaux = ornements(&partition);
    let turn = ornements_initiaux
        .iter()
        .find(|ornement| ornement.kind == OrnamentType::Turn)
        .expect("le corpus porte un turn");
    assert_eq!(turn.interval_above.as_deref(), Some("second,major"));
    assert_eq!(turn.interval_below.as_deref(), Some("second,minor"));
    let notes = partition.notes.len();
    let mesures = partition.measures.len();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<intervalAbove>second,major</intervalAbove>"),
        "l'intervalle supérieur doit être réécrit"
    );
    assert!(
        ecrit.contains("<intervalBelow>second,minor</intervalBelow>"),
        "l'intervalle inférieur doit être réécrit"
    );

    let relu = lire(ecrit.as_bytes());
    let relus = ornements(&relu);
    assert_eq!(relus.len(), ornements_initiaux.len());
    let retour_turn = relus
        .iter()
        .find(|ornement| ornement.kind == OrnamentType::Turn)
        .expect("turn relu");
    assert_eq!(retour_turn.interval_above.as_deref(), Some("second,major"));
    assert_eq!(retour_turn.interval_below.as_deref(), Some("second,minor"));
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

    // Cas non concerné : le trille n'a qu'un intervalle supérieur ; aucun
    // intervalle inférieur ne doit être inventé pour lui.
    let trille = relus
        .iter()
        .find(|ornement| ornement.kind == OrnamentType::TrillMark)
        .expect("trille relu");
    assert_eq!(trille.interval_above.as_deref(), Some("fourth,augmented"));
    assert!(trille.interval_below.is_none());
}
