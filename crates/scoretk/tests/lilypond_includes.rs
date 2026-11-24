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

//! Résolution des `\include` LilyPond à travers la façade.
//!
//! Le résolveur reste du côté appelant (aucune entrée/sortie dans la
//! bibliothèque) : ces tests décrivent le contrat utilisé par le paquet wasm,
//! qui construit le sien depuis l'option additive `includes`.

use std::collections::BTreeMap;

use scoretk::{read, read_lilypond_with_includes, Format};

/// Hauteurs des notes du modèle, dans l'ordre.
fn pitches(score: &scoretk::model::Score) -> Vec<u8> {
    score.notes.iter().map(|note| note.pitch).collect()
}

/// Résolveur de dossier : chemin relatif au fichier incluant, puis racine.
fn resolver<'a>(
    files: &'a BTreeMap<&'static str, &'static str>,
) -> impl FnMut(&str, Option<&str>) -> Option<String> + 'a {
    move |requested, includer| {
        let mut candidates = Vec::new();
        if let Some((directory, _)) = includer.and_then(|path| path.rsplit_once('/')) {
            candidates.push(format!("{directory}/{requested}"));
        }
        candidates.push(requested.to_owned());
        candidates
            .iter()
            .find_map(|path| files.get(path.as_str()))
            .map(|text| (*text).to_owned())
    }
}

#[test]
fn facade_include_racine_donne_des_notes() {
    let files = BTreeMap::from([("noaten.ily", "motif = { c'4 d' e' }")]);
    let source = b"\\version \"2.24.0\"\n\\include \"noaten.ily\"\n{ \\motif }\n";
    let score = read_lilypond_with_includes(source, &mut resolver(&files)).expect("lecture");
    assert_eq!(pitches(&score), vec![60, 62, 64]);
}

#[test]
fn facade_include_relatif_au_fichier_incluant_puis_a_la_racine() {
    let files = BTreeMap::from([
        ("sub/a.ily", "\\include \"b.ily\"\na = { c'4 }"),
        ("b.ily", "b = { d'4 }"),
    ]);
    let source = b"\\version \"2.24.0\"\n\\include \"sub/a.ily\"\n{ \\a \\b e'4 }\n";
    let score = read_lilypond_with_includes(source, &mut resolver(&files)).expect("lecture");
    assert_eq!(pitches(&score), vec![60, 62, 64]);
}

#[test]
fn facade_include_inconnu_garde_le_comportement_sans_resolution() {
    let source = b"\\version \"2.24.0\"\n\\include \"absent.ily\"\n{ c'4 }\n";
    let files = BTreeMap::new();
    let score = read_lilypond_with_includes(source, &mut resolver(&files)).expect("lecture");
    assert_eq!(pitches(&score), vec![60]);
    // Sans table d'inclusions, `read` reste exactement l'ancien chemin.
    let plain = read(source, Format::Ly).expect("lecture");
    assert_eq!(pitches(&score), pitches(&plain));
}
