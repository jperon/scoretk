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

//! MEI 5.1 Neume Notation : le flux L6 grégorien traverse MEI sans perte
//! structurelle, et le chemin CMN reste inchangé pour les autres partitions.

use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::Extension;
use scoretk::{read, write, Format, Options, Score};
use scoretk_mei_export::MeiOptions;

fn gabc_corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("gabc");
    let mut paths: Vec<_> = fs::read_dir(root)
        .expect("Gregorio corpus")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "gabc"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn gabc_crosses_mei_with_the_whole_structured_payload() {
    for path in gabc_corpus() {
        let source = fs::read(&path).expect("GABC source");
        let original = read(&source, Format::Gabc).expect("GABC read");
        let options = MeiOptions {
            gabc_tokens: true,
            ..MeiOptions::default()
        };
        let mei = scoretk_mei_export::write_mei_with(&original, &options)
            .expect("MEI neumes write")
            .into_bytes();
        let text = String::from_utf8(mei.clone()).expect("UTF-8 MEI");
        assert!(text.contains("meiversion=\"5.1\""));
        assert!(text.contains("<syllable>"));
        assert!(text.contains("<nc "));
        assert!(text.contains("type=\"gabc-header\""));
        let from_mei = read(&mei, Format::Mei).expect("MEI neumes read");
        // The NABC manuscript voice is additive: `full_support` is false
        // here, so the second staff is intentionally absent. Compare the
        // rest of the payload, which must cross MEI unchanged.
        let mut expected = original.extensions.format.clone();
        let mut actual = from_mei.extensions.format.clone();
        for payload in [&mut expected, &mut actual] {
            if let Some(Extension::Gregorian(gregorian)) = payload {
                for syllable in &mut gregorian.syllables {
                    syllable.nabc.clear();
                }
            }
        }
        assert_eq!(
            actual,
            expected,
            "{} : the L6 payload must cross MEI unchanged",
            path.display()
        );
        // La vue moderne est reconstruite à partir du payload restauré, pas
        // copiée depuis le MEI (qui n'a ni mesure ni durée pour le chant).
        assert_eq!(from_mei.notes.len(), original.notes.len());
        assert_eq!(from_mei.measures.len(), original.measures.len());
    }
}

#[test]
fn standard_mei_neumes_carry_the_payload_without_any_private_annotation() {
    for path in gabc_corpus() {
        let source = fs::read(&path).expect("GABC source");
        let original = read(&source, Format::Gabc).expect("GABC read");
        let mei = write(&original, Format::Mei, &Options::default()).expect("MEI neumes write");
        let text = String::from_utf8(mei.clone()).expect("UTF-8 MEI");
        assert!(
            !text.contains("<annot type=\"gabc-"),
            "{} : the default MEI must not carry a private GABC payload",
            path.display()
        );
        assert!(
            text.contains("<neume "),
            "{} : neume groups",
            path.display()
        );
        assert!(
            text.contains("<nc "),
            "{} : neume components",
            path.display()
        );
        let Some(Extension::Gregorian(source_gregorian)) = &original.extensions.format else {
            panic!("GABC payload");
        };
        if source_gregorian
            .syllables
            .iter()
            .any(|syllable| !syllable.bars.is_empty())
        {
            assert!(
                text.contains("<divLine "),
                "{} : separation bars",
                path.display()
            );
        }
        assert!(text.contains("<title "), "{} : header", path.display());
        assert!(text.contains("<syl "), "{} : lyrics", path.display());
        let from_mei = read(&mei, Format::Mei).expect("standard MEI neumes read");
        let (Some(Extension::Gregorian(expected)), Some(Extension::Gregorian(actual))) =
            (&original.extensions.format, &from_mei.extensions.format)
        else {
            panic!("both scores carry an L6 payload");
        };
        // Les décalages UTF-16 de la source sont la seule donnée sans
        // porteur MEI 5.1 : ils ne font pas partie de l'inventaire comparé.
        let mut expected = expected.clone();
        let mut actual = actual.clone();
        for gregorian in [&mut expected, &mut actual] {
            for syllable in &mut gregorian.syllables {
                // NABC needs `full_support` (second staff); default MEI is
                // byte-identical to master and carries no second voice.
                syllable.nabc.clear();
                for note in &mut syllable.notes {
                    note.source_offset = None;
                    note.source_end = None;
                }
            }
        }
        assert_eq!(
            actual,
            expected,
            "{} : standard MEI carriers",
            path.display()
        );
        assert_eq!(from_mei.notes.len(), original.notes.len());
        assert_eq!(from_mei.measures.len(), original.measures.len());
    }
}

#[test]
fn cmn_scores_still_use_the_unchanged_mei_basic_path() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("v4");
    let path = fs::read_dir(dir)
        .expect("v4 corpus")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .find(|candidate| candidate.extension().is_some_and(|ext| ext == "mscx"))
        .expect("a v4 corpus score");
    let bytes = fs::read(path).expect("read a v4 corpus score");
    let score: Score = read(&bytes, Format::Mscx).expect("MuseScore read");
    let mei = write(&score, Format::Mei, &Options::default()).expect("CMN MEI write");
    let text = String::from_utf8(mei).expect("UTF-8 MEI");
    assert!(text.contains("meiversion=\"5.1+basic\""));
    assert!(text.contains("<measure"));
    assert!(!text.contains("<syllable"));
    assert!(!text.contains("gabc-"));
}
