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

//! The Gregorian reading dialect must stay opt-in, like the writing one.

use scoretk::{read, read_lilypond_with_dialect, Format};
use scoretk_core::model::Extension;

// Small self-written source: a Vatican voice without any GABC payload or
// `--ly-gregorian` option (LilyPond NR §17.4, `augmentum.ly` shape).
const VATICANA: &str = r#"\version "2.25.5"
\new VaticanaScore {
  \new VaticanaVoice {
    \[ \augmentum a \flexa \augmentum g \]
    \augmentum g
  }
}
"#;

#[test]
fn plain_lilypond_read_keeps_the_ordinary_dialect() {
    let score = read(VATICANA.as_bytes(), Format::Ly).expect("plain .ly import");
    assert!(
        !matches!(score.extensions.format, Some(Extension::Gregorian(_))),
        "the opt-in Gregorian dialect leaked into the default reader"
    );
    assert!(!score.parts.is_empty(), "ordinary notes were not read");
}

#[test]
fn opt_in_lilypond_read_restores_the_gregorian_payload() {
    let score = read_lilypond_with_dialect(VATICANA.as_bytes(), true)
        .expect("Gregorian .ly import");
    assert!(
        matches!(score.extensions.format, Some(Extension::Gregorian(_))),
        "the opt-in Gregorian dialect did not restore the payload"
    );
}

