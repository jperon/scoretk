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

//! Lecture du MusicXML compressé (`.mxl`).
//!
//! Le contrat vérifié ici est celui de la façade : le `.mxl` produit par
//! l'écrivain de `scoretk_core` doit se relire exactement comme le `.musicxml`
//! qu'il contient. Le test parcourt tout le corpus `tests/corpus/{v3,v4}`.

mod common;

use std::io::{Cursor, Write};

use scoretk_musescore_import::MsczContainer;
use scoretk_musicxml_export::{export_musicxml_with, export_mxl_with, ExportOptions};
use scoretk_musicxml_import::{read_musicxml, read_mxl};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn score_from_mscx(mscx: &[u8]) -> scoretk_core::model::Score {
    let container = MsczContainer::from_bytes(mscx).expect("conteneur .mscx");
    scoretk_musescore_import::read::read_score(&container).expect("lecture .mscx")
}

/// Emballe un MusicXML dans une archive `.mxl` donnée.
fn mxl(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let options = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer.start_file(*name, options).expect("entrée ZIP");
        writer.write_all(data).expect("contenu ZIP");
    }
    writer.finish().expect("archive .mxl").into_inner()
}

#[test]
fn aller_retour_mxl_sur_le_corpus() {
    let options = ExportOptions::default();
    let mut files = 0;

    for version in ["v3", "v4"] {
        for path in common::corpus_files(version) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let mscx = std::fs::read(&path).expect("corpus");
            let score = score_from_mscx(&mscx);

            let musicxml = export_musicxml_with(&score, &options).expect("export MusicXML");
            let archive = export_mxl_with(&score, &options).expect("export .mxl");

            let depuis_mxl = read_mxl(&archive).expect("lecture .mxl");
            let depuis_xml = read_musicxml(musicxml.as_bytes()).expect("lecture .musicxml");
            let rendu_mxl = export_musicxml_with(&depuis_mxl, &options).expect("réexport du .mxl");
            let rendu_xml =
                export_musicxml_with(&depuis_xml, &options).expect("réexport du .musicxml");

            assert_eq!(rendu_mxl, rendu_xml, "{version}/{name} : .mxl ≠ .musicxml");
            files += 1;
        }
    }

    assert!(files >= 56, "corpus inattendu : {files} fichiers");
    println!("aller-retour .mxl : {files} fichiers identiques au .musicxml");
}

#[test]
fn mxl_sans_container_xml_se_replie_sur_le_musicxml() {
    let xml = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
  </measure></part>
</score-partwise>"#;
    let archive = mxl(&[("score.musicxml", xml)]);

    assert_eq!(
        read_mxl(&archive).expect("lecture .mxl sans container.xml"),
        read_musicxml(xml).expect("lecture du MusicXML équivalent")
    );
}

#[test]
fn mxl_avec_container_xml_suit_le_rootfile() {
    let xml = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
  </measure></part>
</score-partwise>"#;
    let autre = br#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>
  </measure></part>
</score-partwise>"#;
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container><rootfiles>
  <rootfile full-path="scores/partition.musicxml"/>
  <rootfile full-path="autre.xml"/>
</rootfiles></container>"#;
    let archive = mxl(&[
        ("META-INF/container.xml", container),
        ("scores/partition.musicxml", xml),
        ("autre.xml", autre),
    ]);

    let score = read_mxl(&archive).expect("lecture .mxl");
    assert_eq!(
        score,
        read_musicxml(xml).expect("lecture du premier rootfile")
    );
}

#[test]
fn zip_invalide_est_une_erreur() {
    let error = read_mxl(b"ceci n'est pas une archive").expect_err("ZIP invalide");
    assert!(
        matches!(error, scoretk_musicxml_import::Error::Zip(_)),
        "{error:?}"
    );
}

#[test]
fn archive_sans_musicxml_est_une_erreur() {
    let archive = mxl(&[("META-INF/container.xml", b"<container/>")]);
    let error = read_mxl(&archive).expect_err("archive sans MusicXML");
    assert!(
        matches!(error, scoretk_musicxml_import::Error::NoRootFile),
        "{error:?}"
    );

    let archive = mxl(&[("lisez-moi.txt", b"rien")]);
    let error = read_mxl(&archive).expect_err("archive sans MusicXML");
    assert!(
        matches!(error, scoretk_musicxml_import::Error::NoMusicXmlInMxl),
        "{error:?}"
    );
}

#[test]
fn rootfile_absent_est_une_erreur() {
    let archive = mxl(&[(
        "META-INF/container.xml",
        br#"<container><rootfiles><rootfile full-path="absent.xml"/></rootfiles></container>"#,
    )]);
    let error = read_mxl(&archive).expect_err("rootfile absent");
    assert!(
        matches!(error, scoretk_musicxml_import::Error::RootFileNotFound(ref name) if name == "absent.xml"),
        "{error:?}"
    );
}
