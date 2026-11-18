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

//! Écriture MuseScore par la façade (feature `musescore-export`).
//!
//! Le test complet d'aller-retour sur tout le corpus vit dans
//! `crates/scoretk-musescore-export/tests/roundtrip.rs` ; celui-ci vérifie le
//! branchement de la façade et le contrat `.mscz`.

#![cfg(feature = "musescore-export")]

use std::path::PathBuf;

use scoretk::{read, write, Format, Options};

fn corpus_v4(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v4")
        .join(name)
}

fn options() -> Options {
    Options {
        encoding_date: "2026-11-17".to_owned(),
        ..Options::default()
    }
}

#[test]
fn ecriture_mscx_et_mscz_prise_en_charge() {
    assert!(Format::Mscx.supports_write());
    assert!(Format::Mscz.supports_write());
}

#[test]
fn mscx_relu_donne_le_meme_modele() {
    let input = std::fs::read(corpus_v4("hairpins_and_repeats.mscx")).expect("corpus");
    let score = read(&input, Format::Mscx).expect("lecture");
    let written = write(&score, Format::Mscx, &options()).expect("écriture .mscx");
    let text = String::from_utf8(written).expect("UTF-8");
    assert!(text.contains("<museScore version=\"4.70\">"));
    let reread = read(text.as_bytes(), Format::Mscx).expect("relecture");
    assert_eq!(score, reread, "le modèle doit survivre à l'aller-retour");

    // Même sortie MusicXML de part et d'autre.
    let direct = write(&score, Format::MusicXml, &options()).expect("export direct");
    let round = write(&reread, Format::MusicXml, &options()).expect("export après relecture");
    assert_eq!(direct, round);
}

#[test]
fn mscz_est_un_conteneur_musescore() {
    let input = std::fs::read(corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&input, Format::Mscx).expect("lecture");
    let archive = write(&score, Format::Mscz, &options()).expect("écriture .mscz");

    let container =
        scoretk_musescore_import::MsczContainer::from_bytes(&archive).expect("conteneur");
    assert_eq!(container.version().expect("version").to_string(), "4.70");
    assert_eq!(container.main_mscx_name(), "score.mscx");
    assert_eq!(Format::detect(&archive), Some(Format::Mscz));

    let reread = read(&archive, Format::Mscz).expect("relecture .mscz");
    assert_eq!(score, reread);
}
