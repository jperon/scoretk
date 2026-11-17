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

//! Aiguillage de la façade : parité avec `mscz2xml`, `.mxl` et erreurs des
//! formats non branchés.

mod common;

use std::io::Read;

use mscz2xml::{ConvertOptions, ExportOptions, OutputFormat};
use scoretk::{read, write, Error, Format, Options, Sens};

const ENCODING_DATE: &str = "2026-11-17";

fn options() -> Options {
    Options {
        encoding_date: ENCODING_DATE.to_owned(),
        ..Options::default()
    }
}

fn direct_convert(mscx: &[u8]) -> Vec<u8> {
    let options = ConvertOptions {
        format: OutputFormat::MusicXml,
        export: ExportOptions {
            encoding_date: ENCODING_DATE.to_owned(),
            ..ExportOptions::default()
        },
    };
    mscz2xml::convert(mscx, &options).expect("conversion mscz2xml")
}

#[test]
fn mscz_vers_musicxml_identique_a_mscz2xml() {
    for name in [
        "dynamics.mscx",
        "hairpin_simple.mscx",
        "hairpins_and_repeats.mscx",
    ] {
        let path = common::corpus_v4(name);
        let mscx =
            std::fs::read(&path).unwrap_or_else(|error| panic!("{} : {error}", path.display()));
        let mscz = common::mscz_from_mscx(&mscx);

        assert_eq!(Format::detect(&mscz), Some(Format::Mscz), "{name}");

        let score = read(&mscz, Format::Mscz).expect("lecture du .mscz");
        let via_facade = write(&score, Format::MusicXml, &options()).expect("écriture MusicXML");
        assert_eq!(
            via_facade,
            direct_convert(&mscx),
            "{name} : la façade doit produire exactement la sortie de mscz2xml"
        );
    }
}

#[test]
fn mscx_nu_est_lu_comme_un_mscz() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let from_mscx = read(&mscx, Format::Mscx).expect("lecture .mscx");
    let from_mscz = read(&common::mscz_from_mscx(&mscx), Format::Mscz).expect("lecture .mscz");
    assert_eq!(from_mscx, from_mscz);
}

#[test]
fn mxl_est_une_archive_du_musicxml() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");

    let musicxml = write(&score, Format::MusicXml, &options()).expect("MusicXML");
    let mxl = write(&score, Format::Mxl, &options()).expect(".mxl");

    assert_eq!(Format::detect(&mxl), Some(Format::Mxl));

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(mxl)).expect("archive .mxl");
    let mut container = String::new();
    archive
        .by_name("META-INF/container.xml")
        .expect("container.xml")
        .read_to_string(&mut container)
        .expect("lecture container.xml");
    assert!(container.contains("full-path=\"score.xml\""));

    let mut score_xml = Vec::new();
    archive
        .by_name("score.xml")
        .expect("score.xml")
        .read_to_end(&mut score_xml)
        .expect("lecture score.xml");
    assert_eq!(
        score_xml, musicxml,
        "score.xml contient le MusicXML exporté"
    );
}

#[test]
fn formats_non_branches_renvoient_une_erreur_explicite() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");

    for format in [Format::Ly, Format::Abc, Format::Midi] {
        match read(b"x", format) {
            Err(Error::FormatNonPrisEnCharge {
                format: actual,
                sens,
            }) => {
                assert_eq!(actual, format);
                assert_eq!(sens, Sens::Lecture);
            }
            other => panic!("lecture {format} : {other:?}"),
        }
    }

    for format in [Format::Mscz, Format::Mscx, Format::Ly, Format::Abc] {
        match write(&score, format, &options()) {
            Err(Error::FormatNonPrisEnCharge {
                format: actual,
                sens,
            }) => {
                assert_eq!(actual, format);
                assert_eq!(sens, Sens::Ecriture);
            }
            other => panic!("écriture {format} : {other:?}"),
        }
    }
}

#[cfg(not(feature = "musicxml-import"))]
#[test]
fn lecture_musicxml_branchable_par_feature() {
    let musicxml = br#"<score-partwise version="4.0"/>"#;
    match read(musicxml, Format::MusicXml) {
        Err(Error::FormatNonPrisEnCharge { format, sens }) => {
            assert_eq!(format, Format::MusicXml);
            assert_eq!(sens, Sens::Lecture);
        }
        other => panic!("lecture MusicXML sans feature : {other:?}"),
    }
    assert!(!Format::MusicXml.supports_read());
}

#[cfg(not(feature = "midi"))]
#[test]
fn ecriture_midi_branchable_par_feature() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");
    match write(&score, Format::Midi, &options()) {
        Err(Error::FormatNonPrisEnCharge { format, sens }) => {
            assert_eq!(format, Format::Midi);
            assert_eq!(sens, Sens::Ecriture);
        }
        other => panic!("écriture MIDI sans feature : {other:?}"),
    }
    assert!(!Format::Midi.supports_write());
}

#[cfg(feature = "lilypond")]
#[test]
fn ecriture_lilypond_en_mode_fixed() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");
    let lilypond = write(&score, Format::Ly, &options()).expect("écriture LilyPond");
    let text = String::from_utf8(lilypond).expect("UTF-8");
    assert!(text.starts_with("\\version \"2.24.0\"\n"), "{text}");
    assert!(text.contains("= \\fixed c' {"), "{text}");
    assert!(!text.contains("\\relative"), "{text}");
    assert!(Format::Ly.supports_write());
}

#[cfg(not(feature = "lilypond"))]
#[test]
fn ecriture_lilypond_branchable_par_feature() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");
    match write(&score, Format::Ly, &options()) {
        Err(Error::FormatNonPrisEnCharge { format, sens }) => {
            assert_eq!(format, Format::Ly);
            assert_eq!(sens, Sens::Ecriture);
        }
        other => panic!("écriture LilyPond sans feature : {other:?}"),
    }
    assert!(!Format::Ly.supports_write());
}
