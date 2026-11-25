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

//! Round trip on the nine Gregorio GABC examples.
//!
//! The sources are copied from Gregorio (`doc/*.gabc`, `examples/*.gabc`,
//! commit `b63f074`, GPL-3.0-or-later) as declared in `NOTICE`.

use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::{compare, inventory, Layers};
use scoretk::{read, write, Format, Options};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus")
        .join("gabc")
}

fn examples() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("GABC corpus directory")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gabc")
        })
        .collect();
    paths.sort();
    paths
}

#[test]
fn the_nine_gregorio_examples_are_versioned() {
    assert_eq!(examples().len(), 9);
}

#[test]
fn gabc_to_model_to_gabc_preserves_bytes_and_inventory() {
    let paths = examples();
    assert!(!paths.is_empty());
    for path in paths {
        let source = fs::read(&path).expect("read GABC example");
        let score = read(&source, Format::Gabc)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let written = write(&score, Format::Gabc, &Options::default())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(written, source, "{} changed bytes", path.display());

        let again = read(&written, Format::Gabc).expect("re-read written GABC");
        let reference = inventory(&score);
        let candidate = inventory(&again);
        let coverage = compare(&reference, &candidate, Layers::ALL);
        assert_eq!(
            (
                coverage.total.reference,
                coverage.total.matched,
                coverage.total.extra
            ),
            (reference.items.len(), reference.items.len(), 0),
            "{} changed L6 items",
            path.display()
        );
    }
}
