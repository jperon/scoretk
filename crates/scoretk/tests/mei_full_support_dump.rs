// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : il exporte les 56 partitions du corpus avec les
// porteurs MEI 5.1 additionnels, pour validation RelaxNG par jing.
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

//! Utility test: `SCORETK_MEI_DUMP_DIR=<dir> cargo test -p scoretk --test
//! mei_full_support_dump` writes the 56 corpus exports with
//! `mei_full_support` enabled. Without the variable the test is a no-op.

use std::fs;
use std::path::{Path, PathBuf};

use scoretk::model::inventory;
use scoretk::{read, write, Format, Options};

#[test]
fn dump_mei_full_support_corpus() {
    let Ok(directory) = std::env::var("SCORETK_MEI_DUMP_DIR") else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let mut written = 0;
    for version in ["v3", "v4"] {
        let mut paths: Vec<PathBuf> = fs::read_dir(root.join(version))
            .expect("corpus directory")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "mscx"))
            .collect();
        paths.sort();
        for path in paths {
            let bytes = fs::read(&path).expect("read .mscx reference");
            let score = read(&bytes, Format::Mscx).expect("read MuseScore score");
            let mei = write(&score, Format::Mei, &options).expect("write MEI");
            let name = path.file_stem().expect("score name").to_string_lossy();
            fs::write(
                Path::new(&directory).join(format!("{version}_{name}.mei")),
                mei,
            )
            .expect("write MEI export");
            written += 1;
        }
    }
    assert_eq!(written, 56);
}

/// Debug helper: `SCORETK_MEI_SCORE=v4/linked-ties SCORETK_MEI_KINDS=tie
/// cargo test -p scoretk --test mei_full_support_dump diagnose_one_score --
/// --nocapture` prints the reference and re-imported items of one score.
#[test]
fn diagnose_one_score() {
    let Ok(name) = std::env::var("SCORETK_MEI_SCORE") else {
        return;
    };
    let kinds: Vec<String> = std::env::var("SCORETK_MEI_KINDS")
        .unwrap_or_default()
        .split(',')
        .map(str::to_owned)
        .filter(|kind| !kind.is_empty())
        .collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("corpus");
    let bytes = if let Ok(file) = std::env::var("SCORETK_MEI_FILE") {
        fs::read(file).expect("read MEI file")
    } else {
        let (version, stem) = name.split_once('/').expect("version/name");
        fs::read(root.join(version).join(format!("{stem}.mscx"))).expect("score")
    };
    let score = if std::env::var("SCORETK_MEI_FILE").is_ok() {
        read(&bytes, Format::Mei).expect("read MEI")
    } else {
        read(&bytes, Format::Mscx).expect("read score")
    };
    let options = Options {
        mei_full_support: true,
        ..Default::default()
    };
    let mei = write(&score, Format::Mei, &options).expect("write MEI");
    let back = read(&mei, Format::Mei).expect("read MEI");
    for (label, value) in [("reference", &score), ("reimported", &back)] {
        println!("--- {label}: {name} ---");
        if std::env::var("SCORETK_MEI_CLEFS").is_ok() {
            for (measure_index, measure) in value.measures.iter().enumerate() {
                for (segment_index, segment) in measure.segments.iter().enumerate() {
                    for (track, item) in segment.iter_elements() {
                        if let scoretk::model::SegmentItem::Clef(clef) = item {
                            println!(
                                "    clef segment m{measure_index}/{segment_index} kind {:?} tick {:?} track {} {:?}",
                                segment.kind,
                                segment.tick,
                                track.raw(),
                                clef.concert
                            );
                        }
                    }
                }
            }
        }
        if std::env::var("SCORETK_MEI_SEGMENTS").is_ok() {
            for (measure_index, measure) in value.measures.iter().enumerate() {
                for (segment_index, segment) in measure.segments.iter().enumerate() {
                    for (track, item) in segment.iter_elements() {
                        if let scoretk::model::SegmentItem::ChordRest(cr) = item {
                            println!(
                                "    cr segment m{measure_index}/{segment_index} kind {:?} tick {:?} track {} dur {:?}",
                                segment.kind,
                                segment.tick,
                                track.raw(),
                                cr.duration
                            );
                        }
                    }
                }
            }
        }
        if std::env::var("SCORETK_MEI_MEASURES").is_ok() {
            for measure in &value.measures {
                println!(
                    "    measure start {:?} nominal {:?} tsn {} len {:?}",
                    measure.start_tick,
                    measure.nominal_len,
                    measure.time_sig_numerator,
                    measure.len
                );
            }
        }
        for item in &inventory(value).items {
            if kinds.is_empty() || kinds.contains(&item.kind.to_owned()) {
                println!(
                    "    {:<20} tick {:?} staff {:?} {}",
                    item.kind, item.tick, item.staff, item.value
                );
            }
        }
    }
}
