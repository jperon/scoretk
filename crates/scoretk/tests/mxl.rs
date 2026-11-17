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

//! Lecture `.mxl` par la façade (feature `musicxml-import`).
//!
//! Le `.mxl` écrit par `scoretk` doit se relire comme le `.musicxml`
//! équivalent, et rester distinct d'un `.mscz` à la détection par contenu.

mod common;

#[allow(unused_imports)]
use scoretk::{read, write, Error, Format, Options, Sens};

const ENCODING_DATE: &str = "2026-11-17";

fn options() -> Options {
    Options {
        encoding_date: ENCODING_DATE.to_owned(),
        ..Options::default()
    }
}

#[cfg(feature = "musicxml-import")]
#[test]
fn mxl_se_lit_comme_le_musicxml() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");

    let mxl = write(&score, Format::Mxl, &options()).expect("écriture .mxl");
    let musicxml = write(&score, Format::MusicXml, &options()).expect("écriture MusicXML");
    assert_eq!(Format::detect(&mxl), Some(Format::Mxl));
    assert!(Format::Mxl.supports_read());

    let depuis_mxl = read(&mxl, Format::Mxl).expect("lecture .mxl");
    let depuis_xml = read(&musicxml, Format::MusicXml).expect("lecture .musicxml");
    assert_eq!(
        write(&depuis_mxl, Format::MusicXml, &options()).expect("réexport .mxl"),
        write(&depuis_xml, Format::MusicXml, &options()).expect("réexport .musicxml")
    );
}

#[cfg(feature = "musicxml-import")]
#[test]
fn mxl_sans_container_xml_se_lit_par_repli() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");
    let musicxml = write(&score, Format::MusicXml, &options()).expect("écriture MusicXML");

    // Archive minimale sans `META-INF/container.xml` : le repli prend le
    // premier `.musicxml` hors `META-INF`.
    let archive = common::zip_from_entries(&[("score.musicxml", &musicxml)]);
    assert_eq!(Format::detect(&archive), Some(Format::Mxl));

    let depuis_mxl = read(&archive, Format::Mxl).expect("lecture .mxl");
    assert_eq!(
        write(&depuis_mxl, Format::MusicXml, &options()).expect("réexport"),
        musicxml
    );
}

#[cfg(feature = "musicxml-import")]
#[test]
fn zip_invalide_est_une_erreur_propre() {
    match read(b"PK\x03\x04 pas une archive", Format::Mxl) {
        Err(Error::MusicXmlImport(message)) => {
            assert!(message.contains("conteneur .mxl"), "{message}");
        }
        other => panic!("archive invalide : {other:?}"),
    }
}

#[cfg(not(feature = "musicxml-import"))]
#[test]
fn lecture_mxl_branchable_par_feature() {
    match read(b"PK\x03\x04", Format::Mxl) {
        Err(Error::FormatNonPrisEnCharge { format, sens }) => {
            assert_eq!(format, Format::Mxl);
            assert_eq!(sens, Sens::Lecture);
        }
        other => panic!("lecture .mxl sans feature : {other:?}"),
    }
    assert!(!Format::Mxl.supports_read());
}
