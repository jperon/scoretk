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

//! Aller-retour MusicXML → modèle → MusicXML sur le corpus.
//!
//! Pour chaque `tests/corpus/{v3,v4}/*.ref.musicxml`, on lit la référence avec
//! [`read_musicxml`], on la réexporte avec l'export de `scoretk_core`, et on
//! compare les deux documents après la normalisation de
//! `crates/scoretk/tests/e2e_corpus.rs`, aux deux niveaux :
//!
//! - **noyau musical** (`Level::Core`) : c'est le niveau bloquant, celui de la
//!   conformité `mscx → MusicXML` ;
//! - **parité complète** (`Level::Full`) : directions, articulations, spanners
//!   et crédits sont comparés aussi ; il mesure la couverture du lecteur et
//!   n'échoue que sur [`EXPECTED_FULL_V3`] / [`EXPECTED_FULL_V4`].
//!
//! Le test imprime « aller-retour v3 : N/30 identiques » et la liste des
//! écarts. Seuls les fichiers de [`EXPECTED_V3`] / [`EXPECTED_V4`] doivent
//! rester identiques au niveau noyau : toute régression fait échouer la suite.

mod common;

use std::fs;

use scoretk_musicxml::read_musicxml;
use scoretk_musicxml_export::{export_musicxml_with, ExportOptions};

use common::Level;

/// Date d'encodage figée (le `<identification>` est normalisé de toute façon).
const ENCODING_DATE: &str = "2026-11-16";

/// Fichiers du corpus 3.x dont l'aller-retour est identique à la référence.
///
/// Toute régression sur cette liste fait échouer le test. Les fichiers absents
/// sont suivis dans `docs/agents/musicxml-import.md`.
const EXPECTED_V3: &[&str] = &[
    "Beam-CrossM1",
    "barlinedelete",
    "change",
    "clef-1",
    "copypaste_tuplet_01",
    "grace",
    "keysig_78216",
    "lyricsline01",
    "measure-4",
    "moonlight",
    "repeat01",
    "repeat06",
    "repeat07",
    "repeat11",
    "repeat20",
    "repeat39",
    "selectionfilter1",
    "testBreaksPage",
    "testChordSymbols",
    "testHairpinDynamics",
    "testHarmony6",
    "testMultiInstrumentPart3",
    "testTboxWords1",
    "testWedge4",
    "timesig-05",
    "undoResequenceMeasure",
];

/// Fichiers du corpus 4.x dont l'aller-retour est identique à la référence.
const EXPECTED_V4: &[&str] = &[
    "beamStemDir",
    "changevisibility",
    "copypaste_parts",
    "copypastesymbollist-lyrics",
    "dynamics",
    "dynamics_niente",
    "expression-1",
    "hairpin_dynamic_inside",
    "hairpin_dynamic_jump",
    "hairpin_simple",
    "hairpins_and_repeats",
    "linked-ties",
    "mmrest",
    "partExclusion",
    "part-textlines",
    "repeat65",
    "repeat66",
    "repeat68",
    "repeat69",
    "tenuto_accent",
    "testExcludeInvisibleElements",
    "testLocalTimesig1",
    "testNoteheads2",
    "testTempoPrecision",
    "timesig-09",
    "voiceswitching",
];

/// Fichiers du corpus 3.x identiques au niveau **parité complète**.
///
/// Liste vide tant que le lecteur ne couvre pas les directions et annotations ;
/// elle ne doit que s'allonger.
const EXPECTED_FULL_V3: &[&str] = &[
    "Beam-CrossM1",
    "barlinedelete",
    "change",
    "clef-1",
    "copypaste_tuplet_01",
    "grace",
    "keysig_78216",
    "lyricsline01",
    "measure-4",
    "repeat01",
    "repeat06",
    "repeat07",
    "repeat11",
    "repeat20",
    "repeat39",
    "selectionfilter1",
    "testBreaksPage",
    "testChordSymbols",
    "testHarmony6",
    "testMultiInstrumentPart3",
    "testTboxWords1",
    "timesig-05",
    "undoResequenceMeasure",
];

/// Fichiers du corpus 4.x identiques au niveau **parité complète**.
const EXPECTED_FULL_V4: &[&str] = &[
    "beamStemDir",
    "changevisibility",
    "copypaste_parts",
    "copypastesymbollist-lyrics",
    "dynamics",
    "dynamics_niente",
    "hairpin_dynamic_inside",
    "hairpin_dynamic_jump",
    "hairpin_simple",
    "hairpins_and_repeats",
    "linked-ties",
    "mmrest",
    "partExclusion",
    "part-textlines",
    "repeat69",
    "tenuto_accent",
    "testExcludeInvisibleElements",
    "testLocalTimesig1",
    "testNoteheads2",
    "testTempoPrecision",
    "timesig-09",
];

/// Résultat d'un aller-retour.
struct Row {
    name: String,
    changed: usize,
    tags: Vec<(String, usize)>,
    diff: String,
    error: Option<String>,
}

fn run(version: &str, level: Level, expected: &[&str]) {
    let files = common::corpus_files(version);
    let mut rows: Vec<Row> = Vec::new();

    let options = ExportOptions {
        encoding_date: ENCODING_DATE.to_owned(),
        ..ExportOptions::default()
    };

    for path in &files {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("nom de fichier UTF-8")
            .to_owned();
        let reference_path = path.with_file_name(format!("{name}.ref.musicxml"));
        let reference = fs::read_to_string(&reference_path).unwrap_or_else(|error| {
            panic!("référence {} illisible : {error}", reference_path.display())
        });
        let expected_lines = common::normalize_with(&reference, level);

        let input = fs::read(&reference_path).expect("lecture de la référence");
        match read_musicxml(&input) {
            Ok(score) => match export_musicxml_with(&score, &options) {
                Ok(xml) => {
                    let actual = common::normalize_with(&xml, level);
                    let (changed, diff) = common::diff(&expected_lines, &actual);
                    rows.push(Row {
                        name,
                        changed,
                        tags: common::changed_tags(&expected_lines, &actual),
                        diff: common::truncate(&diff, 40),
                        error: None,
                    });
                }
                Err(error) => rows.push(Row {
                    name,
                    changed: usize::MAX,
                    tags: Vec::new(),
                    diff: String::new(),
                    error: Some(format!("export : {error}")),
                }),
            },
            Err(error) => rows.push(Row {
                name,
                changed: usize::MAX,
                tags: Vec::new(),
                diff: String::new(),
                error: Some(format!("lecture : {error}")),
            }),
        }
    }

    let identical: Vec<&str> = rows
        .iter()
        .filter(|row| row.changed == 0)
        .map(|row| row.name.as_str())
        .collect();
    let label = match level {
        Level::Core => "noyau",
        Level::Full => "complet",
    };
    println!(
        "aller-retour {version} ({label}) : {}/{} identiques",
        identical.len(),
        rows.len()
    );
    for row in &rows {
        if let Some(error) = &row.error {
            println!("  {} : ERREUR ({error})", row.name);
        } else if row.changed == 0 {
            println!("  {} : identique", row.name);
        } else {
            let tags: Vec<String> = row
                .tags
                .iter()
                .take(4)
                .map(|(tag, count)| format!("{tag}×{count}"))
                .collect();
            println!(
                "  {} : {} lignes d'écart [{}]",
                row.name,
                row.changed,
                tags.join(", ")
            );
        }
    }
    for row in &rows {
        if row.changed > 0 {
            println!(
                "\n=== {version}/{} : {} ===\n{}",
                row.name,
                if row.changed == usize::MAX {
                    "ERREUR".to_owned()
                } else {
                    format!("{} lignes d'écart", row.changed)
                },
                row.diff
            );
        }
    }

    for name in expected {
        assert!(
            identical.contains(name),
            "{name} est attendu identique en aller-retour ({label}) mais diffère"
        );
    }
}

#[test]
fn aller_retour_v3() {
    run("v3", Level::Core, EXPECTED_V3);
}

#[test]
fn aller_retour_v4() {
    run("v4", Level::Core, EXPECTED_V4);
}

#[test]
fn aller_retour_complet_v3() {
    run("v3", Level::Full, EXPECTED_FULL_V3);
}

#[test]
fn aller_retour_complet_v4() {
    run("v4", Level::Full, EXPECTED_FULL_V4);
}
