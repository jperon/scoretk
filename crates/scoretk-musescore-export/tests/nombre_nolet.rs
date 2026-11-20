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

//! Écriture du type d'affichage du nombre d'un n-olet pour MuseScore 4.6+.

use std::fs;
use std::path::Path;

use scoretk_core::model::{Score, TupletNumberType};
use scoretk_musescore_import::MsczContainer;

fn lire(mscx: &[u8]) -> Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur");
    scoretk_musescore_import::read::read_score(&container).expect("lecture")
}

/// `TWrite::write(const Tuplet*)` (`write/twrite.cpp:3500-3525`) appelle
/// `writeStyledProperties` (`:644-647`) : `Pid::NUMBER_TYPE` est une propriété
/// de style (`dom/tuplet.cpp:50`) nommée `numberType`
/// (`dom/property.cpp:119`). Le lecteur 4.6+ (`read460/tread.cpp:1530`)
/// n'utilise que cette propriété (`Tuplet::numberType()`), et l'export
/// MusicXML de MuseScore n'écrit `show-number="none"`/`"both"` qu'à partir
/// d'elle (`exportmusicxml.cpp:2781-2784`). Sans `<numberType>`, MuseScore
/// relit le type par défaut `SHOW_NUMBER` et perd l'affichage du corpus.
#[test]
fn type_nombre_nolet_ecrit_et_relu() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v3");
    let source = fs::read(racine.join("testTuplets8.mscx")).expect("corpus");
    let mut partition = lire(&source);
    assert!(
        partition.tuplets.len() >= 2,
        "le corpus doit porter plusieurs n-olets"
    );
    let notes = partition.notes.len();
    let mesures = partition.measures.len();
    // Musique ordinaire conservée autour des n-olets : barres, voix, accords
    // et silences du corpus restent intacts ; on ne change que l'affichage.
    partition.tuplets[0].number_type = TupletNumberType::None;
    partition.tuplets[1].number_type = TupletNumberType::Ratio;

    let ecrit = scoretk_musescore_export::write_mscx(&partition).expect("écriture");
    assert!(
        ecrit.contains("<numberType>2</numberType>"),
        "un n-olet sans nombre doit écrire son type (NO_TEXT)"
    );
    assert!(
        ecrit.contains("<numberType>1</numberType>"),
        "un n-olet en ratio doit écrire son type (SHOW_RELATION)"
    );

    // Le lecteur 4.x de scoretk doit relire la propriété, y compris pour un
    // fichier écrit par MuseScore où `<Number>` peut rester vide ou absent.
    let relu = lire(ecrit.as_bytes());
    assert_eq!(relu.tuplets[0].number_type, TupletNumberType::None);
    assert_eq!(relu.tuplets[1].number_type, TupletNumberType::Ratio);
    let retour = scoretk_musicxml_export::export_musicxml(&relu).expect("export");
    assert!(
        retour.contains("show-number=\"none\""),
        "le type relu doit produire show-number=\"none\""
    );
    assert!(
        retour.contains("show-number=\"both\""),
        "le type relu doit produire show-number=\"both\""
    );

    // Aucune perte musicale : hauteurs, durées et mesures sont conservées.
    let direct = scoretk_musicxml_export::export_musicxml(&partition).expect("export");
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

    // Cas non concerné : le type par défaut n'est pas écrit (c'est le défaut
    // de style de MuseScore), et le corpus inchangé garde ses n-olets.
    let intact = lire(&source);
    let ecrit_intact = scoretk_musescore_export::write_mscx(&intact).expect("écriture");
    assert!(
        !ecrit_intact.contains("<numberType>"),
        "aucun <numberType> ne doit être inventé pour le type par défaut"
    );
    assert_eq!(intact.tuplets.len(), partition.tuplets.len());
}
