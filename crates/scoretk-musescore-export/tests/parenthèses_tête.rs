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

//! Propriété `HAS_PARENTHESES` de la tête de note, relue par MuseScore 4.6+.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::writeProperties(const Note*)` (`write/twrite.cpp:690-697`) écrit
/// `Pid::HAS_PARENTHESES` (`<parentheses>both</parentheses>`, table
/// `typesconv.cpp:952-958`) quand la note porte des parenthèses de tête. Le
/// seul `<Symbol name="noteheadParenthesis">` n'est pas reconnu par l'export
/// MusicXML de MuseScore (`importmusicxml.cpp:3883-3902`), qui n'écrit alors pas
/// `<notehead parentheses="yes">`.
#[test]
fn parenthèses_de_tête_ecrites_pour_musescore() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("layout_elements.mscx")).expect("corpus");
    let partition = lire(&source);
    let parents_avant = partition.notes.iter().filter(|n| n.parentheses).count();
    assert!(
        parents_avant > 0,
        "le corpus doit porter une tête entre parenthèses"
    );

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<parentheses>both</parentheses>"),
        "les parenthèses de tête doivent être écrites comme propriété de la note"
    );
    // Le <Symbol> reste pour le lecteur de scoretk (relecture sans perte).
    let relu = lire(ecrit.as_bytes());
    assert_eq!(
        relu.notes.iter().filter(|n| n.parentheses).count(),
        parents_avant
    );

    // Cas non concerné : une partition sans tête entre parenthèses n'invente
    // pas de propriété, et le reste de la musique est inchangé.
    let nu = fs::read(racine.join("barline01.mscx")).expect("corpus");
    let nu_partition = lire(&nu);
    assert_eq!(nu_partition.notes.iter().filter(|n| n.parentheses).count(), 0);
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu_partition).expect("écriture");
    assert!(!ecrit_nu.contains("<parentheses>both</parentheses>"));
    let direct = scoretk_musicxml_export::export_musicxml(&nu_partition).expect("export");
    let retour = scoretk_musicxml_export::export_musicxml(&lire(ecrit_nu.as_bytes())).expect("export");
    assert_eq!(direct.matches("<pitch>").count(), retour.matches("<pitch>").count());
    assert_eq!(direct.matches("<measure ").count(), retour.matches("<measure ").count());
}
