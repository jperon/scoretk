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

//! Façade ABC (feature `abc`) : lecture, écriture et aller-retour.

#![cfg(all(feature = "abc-import", feature = "abc-export"))]

mod common;

use scoretk::{read, write, Format, Options};

const AIR: &str = "\
X:1
T:Air de test
M:4/4
L:1/8
K:G
G A B c | d4 z4 |
";

fn options() -> Options {
    Options {
        encoding_date: "2026-11-17".to_owned(),
        ..Options::default()
    }
}

#[test]
fn lecture_abc_vers_musicxml() {
    let score = read(AIR.as_bytes(), Format::Abc).expect("lecture ABC");
    assert_eq!(score.metadata.work_title, "Air de test");
    let xml = write(&score, Format::MusicXml, &options()).expect("écriture MusicXML");
    let text = String::from_utf8(xml).expect("UTF-8");
    assert!(text.contains("<score-partwise"));
    assert!(text.contains("<step>G</step>"));
    assert!(text.contains("<step>B</step>"));
}

#[test]
fn mscx_vers_abc_puis_retour() {
    let mscx = std::fs::read(common::corpus_v4("dynamics.mscx")).expect("corpus");
    let score = read(&mscx, Format::Mscx).expect("lecture .mscx");
    let abc = write(&score, Format::Abc, &options()).expect("écriture ABC");
    let text = String::from_utf8(abc).expect("UTF-8");
    assert!(text.starts_with("%abc-2.1\nX:1\n"), "{text}");
    assert!(text.contains("\nK:"), "{text}");
    assert!(text.contains("!p!"), "{text}");

    let relu = read(text.as_bytes(), Format::Abc).expect("relecture ABC");
    let direct = write(&score, Format::MusicXml, &options()).expect("MusicXML direct");
    let retour = write(&relu, Format::MusicXml, &options()).expect("MusicXML retour");
    // Le noyau musical est comparé en détail par les crates `scoretk-abc-*` ;
    // ici on vérifie que la façade ne perd ni parties ni notes.
    let direct = String::from_utf8(direct).expect("UTF-8");
    let retour = String::from_utf8(retour).expect("UTF-8");
    assert_eq!(
        direct.matches("<note").count(),
        retour.matches("<note").count(),
        "nombre de notes instable"
    );
}

#[test]
fn abc_invalide_renvoie_une_erreur() {
    let error = read(b"X:1\nM:4/4\nL:1/8\nC D E F |", Format::Abc).expect_err("ABC invalide");
    assert!(error.to_string().contains("ABC"), "{error}");
}

#[test]
fn formats_abc_actifs() {
    assert!(Format::Abc.supports_read());
    assert!(Format::Abc.supports_write());
}
