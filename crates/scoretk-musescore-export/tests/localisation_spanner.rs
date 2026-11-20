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

//! Position des spanners en nombre de mesures dans le `.mscx`.

use std::fs;
use std::path::Path;

use scoretk_core::model::{Score, SpannerType};
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `SpannerWriter::fillSpannerPosition` (`write/connectorinfowriter.cpp:82-96`)
/// range `Location::measure()` et `Location::frac()` séparément ;
/// `TWrite::write(const Location*)` (`write/twrite.cpp:2355-2368`) écrit
/// `<measures>` et `<fractions>` comme deux balises distinctes. Écrire un
/// décalage d'une mesure entière comme `<fractions>1/1</fractions>` fait
/// raccourcir l'ottava d'une mesure à la réouverture par MuseScore 4.7.4
/// (`ConnectorInfoReader::readAddConnector`, `read460/connectorinforeader.cpp:346`).
#[test]
fn ottava_ecrit_ses_mesures_et_non_pas_une_fraction() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("testExcludeInvisibleElements.mscx")).expect("corpus");
    let partition = lire(&source);
    let ottavas: Vec<_> = partition
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Ottava(_)))
        .collect();
    assert_eq!(ottavas.len(), 2, "le corpus doit porter deux ottavas");
    let notes = partition.notes.len();
    let mesures = partition.measures.len();
    let debuts: Vec<_> = ottavas.iter().map(|spanner| spanner.start.tick).collect();
    let fins: Vec<_> = ottavas.iter().map(|spanner| spanner.end.tick).collect();

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<measures>1</measures>"),
        "une ottava d'une mesure doit écrire <measures>"
    );
    assert!(
        !ecrit.contains("<fractions>1/1</fractions>"),
        "un décalage entier ne doit pas être écrit comme fraction"
    );

    let relu = lire(ecrit.as_bytes());
    let relus: Vec<_> = relu
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Ottava(_)))
        .collect();
    assert_eq!(relus.len(), ottavas.len());
    let debuts_relus: Vec<_> = relus.iter().map(|spanner| spanner.start.tick).collect();
    let fins_relues: Vec<_> = relus.iter().map(|spanner| spanner.end.tick).collect();
    assert_eq!(debuts_relus, debuts);
    assert_eq!(fins_relues, fins);
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

    // Cas non concerné : les liaisons intra-mesure restent des fractions.
    let ties = fs::read(racine.join("linked-ties.mscx")).expect("corpus");
    let ties_score = lire(&ties);
    let ecrit_ties = scoretk_musescore_export::write_mscx(&ties_score).expect("écriture");
    assert!(
        ecrit_ties.contains("<fractions>1/4</fractions>"),
        "une liaison d'un quart de mesure reste une fraction"
    );
    assert!(!ecrit_ties.contains("<measures>"));
}
