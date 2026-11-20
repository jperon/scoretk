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

//! Écriture des notes « dead » pour le réexporteur MusicXML de MuseScore.

use std::fs;
use std::path::Path;

use scoretk_core::model::Score;
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const Note*)` (`write/twrite.cpp:2500-2505`) écrit les deux
/// propriétés `Pid::GHOST` (`<ghost>`) et `Pid::DEAD` (`<dead>`) séparément.
/// Le modèle pivot les confond en `Note::ghost` et l'export MusicXML de
/// scoretk ne met pas de parenthèses ; écrire `<ghost>` fait au contraire
/// l'export MuseScore ajouter `<notehead parentheses="yes">` (`ghost()` dans
/// `engravingitem.cpp:1688`). Écrire `<dead>` conserve la parité.
#[test]
fn note_dead_ecrite_comme_dead_pas_comme_ghost() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let source = fs::read(racine.join("testNoteheads2.mscx")).expect("corpus");
    let partition = lire(&source);
    let fantomes = partition.notes.iter().filter(|n| n.ghost).count();
    assert!(fantomes > 0, "le corpus doit porter des notes dead");

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<dead>1</dead>"),
        "les notes dead doivent être écrites comme <dead>"
    );
    assert!(
        !ecrit.contains("<ghost>1</ghost>"),
        "aucune note ne doit être écrite comme <ghost>"
    );
    // Relecture par scoretk sans perte du drapeau, et musique inchangée.
    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.notes.iter().filter(|n| n.ghost).count(), fantomes);
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

    // Cas non concerné : une musique sans note dead n'invente pas de <dead>.
    let nu = fs::read(racine.join("testExcludeInvisibleElements.mscx")).expect("corpus");
    let nu_partition = lire(&nu);
    assert_eq!(nu_partition.notes.iter().filter(|n| n.ghost).count(), 0);
    let ecrit_nu = scoretk_musescore_export::write_mscx(&nu_partition).expect("écriture");
    assert!(!ecrit_nu.contains("<dead>1</dead>"));
}
