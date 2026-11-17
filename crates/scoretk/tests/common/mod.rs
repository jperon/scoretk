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

//! Helpers partagés par les tests d'intégration de `scoretk`.

#![allow(dead_code)]

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Chemin d'un fichier du corpus 4.x.
pub fn corpus_v4(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v4")
        .join(name)
}

/// Chemin d'un fichier du corpus 3.x.
pub fn corpus_v3(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v3")
        .join(name)
}

/// Emballe un `.mscx` dans un `.mscz` minimal, avec `META-INF/container.xml`
/// désignant `score.mscx`.
pub fn mscz_from_mscx(mscx: &[u8]) -> Vec<u8> {
    let options = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("META-INF/container.xml", options)
        .expect("entrée container.xml");
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<container><rootfiles><rootfile full-path="score.mscx"/></rootfiles></container>"#,
        )
        .expect("contenu container.xml");
    writer
        .start_file("score.mscx", options)
        .expect("entrée .mscx");
    writer.write_all(mscx).expect("contenu .mscx");
    writer.finish().expect("archive .mscz").into_inner()
}

/// Emballe un MusicXML dans un `.mxl` de test (nom d'entrée arbitraire).
pub fn mxl_from_musicxml(xml: &[u8]) -> Vec<u8> {
    let options = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("META-INF/container.xml", options)
        .expect("entrée container.xml");
    writer
        .write_all(
            br#"<container><rootfiles><rootfile full-path="score.xml"/></rootfiles></container>"#,
        )
        .expect("contenu container.xml");
    writer
        .start_file("score.xml", options)
        .expect("entrée score.xml");
    writer.write_all(xml).expect("contenu MusicXML");
    writer.finish().expect("archive .mxl").into_inner()
}

/// Référence MIDI du corpus (`tests/corpus/midi/<version>/<nom>`).
pub fn corpus_midi(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("midi")
        .join(name)
}
