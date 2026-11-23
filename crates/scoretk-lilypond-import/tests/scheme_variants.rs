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
// this program. If not, see <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Variantes Scheme sémantiquement équivalentes.
//!
//! Chaque paire `NN-nom.orig.ly` / `NN-nom.variant.ly` produit la **même**
//! musique : la variante réécrit une forme Scheme comprise par le lecteur en
//! une forme équivalente (`let`, `if`, fonction auxiliaire, nombre calculé,
//! quasiquote, liste construite…) que le lecteur doit aussi comprendre. Les
//! deux fichiers ont été compilés par LilyPond 2.27 et donnent le même MIDI :
//! la réécriture est vérifiée sur la référence, pas supposée.
//!
//! Le test compare le modèle pivot complet des deux lectures. Les échecs
//! connus sont listés dans `EXPECTED_FAILURES` : le test échoue si un fichier
//! change de camp (nouvel échec = régression, échec disparu = mettre la liste
//! à jour). C'est la mesure de robustesse qui précède tout évaluateur Scheme
//! (`docs/agents/lilypond-scheme.md`, niveaux A/B).

use std::fs;
use std::path::{Path, PathBuf};

use scoretk_lilypond_import::read_lilypond;

/// Variantes que le lecteur actuel ne lit pas encore comme l'original.
///
/// Mesuré avant tout évaluateur Scheme ; chaque entrée est une forme que
/// l'analyse par motifs ne reconnaît pas.
const EXPECTED_FAILURES: &[&str] = &[
    // Corps de fonction musicale réduit à une expression Scheme générale.
    "01-body-return-let",
    // Nombres calculés là où une constante est attendue.
    "04-repeat-computed",
    "05-tempo-computed",
    "06-measure-length-sum",
    "12-tremolo-repeat-computed",
    "13-duration-multiplier",
    // Corps de fonction musicale sous `if`.
    "14-body-if-music",
];

fn variantes() -> Vec<(String, PathBuf, PathBuf)> {
    let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scheme_variants");
    let mut paires = Vec::new();
    let mut entrees: Vec<PathBuf> = fs::read_dir(&dossier)
        .expect("dossier des variantes")
        .filter_map(|entree| entree.ok().map(|entree| entree.path()))
        .filter(|chemin| {
            chemin
                .file_name()
                .and_then(|nom| nom.to_str())
                .is_some_and(|nom| nom.ends_with(".orig.ly"))
        })
        .collect();
    entrees.sort();
    for orig in entrees {
        let nom = orig.file_name().unwrap().to_str().unwrap();
        let racine = nom.trim_end_matches(".orig.ly").to_owned();
        let variante = orig.with_file_name(format!("{racine}.variant.ly"));
        assert!(variante.exists(), "variante manquante pour {nom}");
        paires.push((racine, orig, variante));
    }
    assert!(paires.len() >= 10, "suite de variantes trop courte");
    paires
}

#[test]
fn variantes_scheme_equivalentes() {
    let mut echecs = Vec::new();
    let mut total = 0usize;
    let mut rapport = String::new();
    for (nom, orig, variante) in variantes() {
        total += 1;
        let source_orig = fs::read_to_string(&orig).expect("lecture de l'original");
        let source_var = fs::read_to_string(&variante).expect("lecture de la variante");
        let modele_orig = read_lilypond(&source_orig);
        let modele_var = read_lilypond(&source_var);
        let identique = match (&modele_orig, &modele_var) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if identique {
            rapport.push_str(&format!("{nom} : identique\n"));
        } else {
            let raison = match (&modele_orig, &modele_var) {
                (Err(e), _) => format!("original en erreur : {e}"),
                (_, Err(e)) => format!("variante en erreur : {e}"),
                (Ok(_), Ok(_)) => "modèles différents".to_owned(),
            };
            rapport.push_str(&format!("{nom} : ÉCHEC ({raison})\n"));
            echecs.push(nom);
        }
    }
    let attendus: Vec<String> = EXPECTED_FAILURES
        .iter()
        .map(|nom| (*nom).to_owned())
        .collect();
    assert_eq!(
        echecs, attendus,
        "échecs inattendus (liste attendue : {EXPECTED_FAILURES:?})\n{rapport}"
    );
    println!(
        "{total} variantes, {} identiques, {} échecs\n{rapport}",
        total - echecs.len(),
        echecs.len()
    );
}
