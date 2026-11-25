// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la syntaxe GABC de Gregorio (GPL-3.0-or-later) : `doc/Gabc.tex`,
// modifié (réécrit en Rust) en 2026.
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

//! GABC writer: pivot `Score` → in-memory text, no I/O.
//!
//! The primary data of GABC is the L6 syllable/neume stream; durations,
//! metre and concert pitch in the pivot are a derived projection
//! (`docs/agents/model-modularisation.md` §7.1). When the payload is present
//! the writer emits its exact source, including comments and spaces, which
//! are significant (`Gabc.tex:60-64,668-687`). Without a payload it refuses
//! to invent chant from an ordinary score rather than approximating one.

use scoretk_core::model::{Capabilities, Extension, Layers, Score};
use scoretk_gabc_common::{Error, Result};

/// Layers GABC carries (`Gabc.tex:228-343,408-443,452-870`; plan §7.1).
/// GABC has no playback data (`:67-154`) and no score-wide layout (`:801-845`).
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::NONE,
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::EXTENSIONS),
);

/// Writes a GABC file from the L6 payload.
///
/// `header_source` ends just before the `%%` line and `notation` starts at
/// that line, so their concatenation is byte-identical to the source that
/// produced the payload. This is what the Gregorio oracle measures.
pub fn write_gabc(score: &Score) -> Result<String> {
    let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
        return Err(Error::MissingGregorian);
    };
    Ok(format!("{}{}", gregorian.header_source, gregorian.notation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Gregorian, Layers};

    #[test]
    fn capabilities_are_the_mirror_of_the_reader() {
        assert_eq!(CAPABILITIES.reads, Layers::NONE);
        assert!(CAPABILITIES.writes.contains(Layers::EXTENSIONS));
        assert!(!CAPABILITIES.writes.contains(Layers::PLAYBACK));
    }

    #[test]
    fn payload_is_written_back_verbatim() {
        let source = "% comment\nname: Veni;\n%%\n(c3) VE(g)ni(h) (::)\n";
        let mut score = Score::new();
        let gregorian = Gregorian {
            header_source: "% comment\nname: Veni;\n".to_owned(),
            notation: "%%\n(c3) VE(g)ni(h) (::)\n".to_owned(),
            ..Gregorian::default()
        };
        score.extensions.format = Some(Extension::Gregorian(gregorian));
        assert_eq!(write_gabc(&score).expect("GABC output"), source);
    }

    #[test]
    fn a_score_without_a_payload_is_refused() {
        assert_eq!(write_gabc(&Score::new()), Err(Error::MissingGregorian));
    }
}
