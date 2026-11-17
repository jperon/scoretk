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

//! Détection de format : extension, nom de fichier et contenu.

mod common;

use scoretk::Format;

#[test]
fn extensions_reconnues_sans_casse() {
    for (extension, expected) in [
        ("mscz", Format::Mscz),
        ("MSCX", Format::Mscx),
        ("musicxml", Format::MusicXml),
        ("xml", Format::MusicXml),
        ("MXL", Format::Mxl),
        ("ly", Format::Ly),
        ("ABC", Format::Abc),
        ("mid", Format::Midi),
        ("midi", Format::Midi),
    ] {
        assert_eq!(
            Format::from_extension(extension),
            Some(expected),
            "extension {extension}"
        );
    }
    assert_eq!(Format::from_extension("pdf"), None);
}

#[test]
fn chemin_et_nom_de_format() {
    assert_eq!(Format::from_path("/tmp/partition.MSCZ"), Some(Format::Mscz));
    assert_eq!(Format::from_path("score.musicxml"), Some(Format::MusicXml));
    assert_eq!(Format::from_path("score"), None);

    assert_eq!("musicxml".parse::<Format>().ok(), Some(Format::MusicXml));
    assert_eq!("MID".parse::<Format>().ok(), Some(Format::Midi));
    let error = "bmp".parse::<Format>().unwrap_err();
    assert!(error.to_string().contains("format inconnu"));
}

#[test]
fn contenu_mscx_et_musicxml() {
    let mscx = br#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.60"><Score/></museScore>"#;
    assert_eq!(Format::detect(mscx), Some(Format::Mscx));

    let musicxml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0"><part-list/></score-partwise>"#;
    assert_eq!(Format::detect(musicxml), Some(Format::MusicXml));

    // Sans déclaration XML.
    assert_eq!(
        Format::detect(br#"<score-partwise version="4.0"/>"#),
        Some(Format::MusicXml)
    );
    // BOM et espaces en tête.
    let mut with_bom = b"\xEF\xBB\xBF  \n".to_vec();
    with_bom.extend_from_slice(br#"<museScore version="3.02"/>"#);
    assert_eq!(Format::detect(&with_bom), Some(Format::Mscx));
}

#[test]
fn contenu_zip_mscz_et_mxl() {
    let mscx = br#"<museScore version="4.60"><Score/></museScore>"#;
    let mscz = common::mscz_from_mscx(mscx);
    assert_eq!(Format::detect(&mscz), Some(Format::Mscz));

    let mxl = common::mxl_from_musicxml(br#"<score-partwise version="4.0"/>"#);
    assert_eq!(Format::detect(&mxl), Some(Format::Mxl));

    // `META-INF/container.xml` seul suffit à reconnaître un `.mxl` : c'est la
    // marque de la spécification MusicXML compressé. Un `.mscz` en contient un
    // aussi, mais le `.mscx` qu'il référence le classe en premier.
    let container_seul = common::zip_from_entries(&[(
        "META-INF/container.xml",
        br#"<container><rootfiles/></container>"#,
    )]);
    assert_eq!(Format::detect(&container_seul), Some(Format::Mxl));
}

#[test]
fn contenu_midi_lilypond_abc() {
    assert_eq!(Format::detect(b"MThd\x00\x00\x00\x06"), Some(Format::Midi));
    assert_eq!(
        Format::detect(b"\\version \"2.24.0\"\n{ c' }"),
        Some(Format::Ly)
    );
    assert_eq!(
        Format::detect(b"%abc-2.1\nX:1\nT:Test\n"),
        Some(Format::Abc)
    );
    assert_eq!(Format::detect(b"X:1\nT:Test\n"), Some(Format::Abc));
}

#[test]
fn contenu_inconnu() {
    assert_eq!(Format::detect(b""), None);
    assert_eq!(Format::detect(b"\x89PNG\r\n\x1a\n"), None);
}
