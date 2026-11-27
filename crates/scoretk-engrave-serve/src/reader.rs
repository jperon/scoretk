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

//! Reading a score from disk, with the CLI's format rules.
//!
//! The facade is linked with the seven import features only: this library
//! never builds an export backend, a MIDI player or an SVG string. The source
//! bytes are dropped on return, before the engraver is built.

use std::path::{Component, Path, PathBuf};

use scoretk::{Format, Score};

/// Reads one file into the pivot model. Identical to `read_score` in
/// `crates/scoretk-cli/src/main.rs`: extension first, then content sniffing;
/// LilyPond includes are resolved relative to the including file and to the
/// main file; all non-Gregorian dialects are the defaults.
pub(crate) fn read_score(path: &str) -> Result<Score, String> {
    let file = Path::new(path);
    let bytes = std::fs::read(file).map_err(|error| format!("cannot read {path}: {error}"))?;
    let format = Format::from_path(file)
        .or_else(|| Format::detect(&bytes))
        .filter(|format| *format != Format::Svg && format.supports_read())
        .ok_or_else(|| format!("unsupported input format for {path}"))?;

    let score = if format == Format::Ly {
        let mut resolve = lilypond_resolver(file);
        scoretk::read_lilypond_with_includes_and_dialect(&bytes, &mut resolve, false)
    } else {
        scoretk::read(&bytes, format)
    };
    score.map_err(|error| format!("{path}: {error}"))
}

/// Resolves a LilyPond `\include`: first next to the including file, then
/// next to the main file, with `.` and `..` normalised lexically. Same rule
/// as `lilypond_resolver` in the CLI.
fn lilypond_resolver(main: &Path) -> impl FnMut(&str, Option<&str>) -> Option<String> {
    let main_dir = main.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    move |requested, includer| {
        let includer_dir = includer
            .and_then(|path| Path::new(path).parent())
            .unwrap_or_else(|| Path::new(""));
        let candidates = [
            main_dir.join(includer_dir).join(requested),
            main_dir.join(requested),
        ];
        candidates
            .iter()
            .find_map(|path| std::fs::read_to_string(normalize_path(path)).ok())
    }
}

/// Normalises a path by resolving `.` and `..` without touching the disk.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
