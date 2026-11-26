// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de la syntaxe GABC de Gregorio (GPL-3.0-or-later) : `doc/Gabc.tex`,
// `src/gabc/gabc-score-determination.{y,l}`, `src/struct.{c,h}`, modifié
// (réécrit en Rust) en 2026.
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

//! GABC reader: in-memory text → pivot `Score`, with no I/O.
//!
//! The L6 [`Gregorian`](scoretk_core::model::Gregorian) payload owns the
//! primary data (syllables, pitch letters, neume glyphs, signs, separation
//! bars and rubrics). The L0/L1 event spine is a **derived projection**,
//! flagged in `Score::report` and in the payload
//! (`docs/agents/model-modularisation.md` §7.1).

mod derived;
mod nabc;
mod read;

use scoretk_core::model::{Capabilities, Layers, Score};
use scoretk_gabc_common::{Error, Result};

/// Layers GABC carries, from the specification (not from what is implemented
/// today): L0 pitch letters and the event spine (`Gabc.tex:408-443`), L1
/// clefs and separation bars (`:801-870`), L2 neumes and rhythmic signs
/// (`:452-797`), L3 syllable text and rubrics (`:228-343`), and the L6
/// Gregorian payload (plan §7.1).
///
/// GABC declares neither L4 (no tempo or velocity header, `:67-154`) nor L5
/// (no page geometry or metric layout, `:801-845`).
pub const CAPABILITIES: Capabilities = Capabilities::new(
    Layers::COMMON
        .union(Layers::METRIC)
        .union(Layers::EVENTS)
        .union(Layers::TEXT)
        .union(Layers::EXTENSIONS),
    Layers::NONE,
);

/// Reads GABC text into the pivot model.
///
/// The two sections are separated by a `%%` line (`Gabc.tex:51,58`). The exact
/// source of both sections is kept in the L6 payload so the writer can emit
/// them back verbatim; the structured fields are the comparable items.
///
/// A file with no `%%` line is accepted as a **headerless body** when its text
/// starts with a clef group or a syllable followed by a note group (raw
/// GregoBase exports, rendered by exsurge). Gregorio itself rejects that
/// input; the tolerance is scoretk's and is documented in
/// `docs/agents/gabc.md` §15. Files with a `%%` line are read exactly as
/// before.
pub fn read_gabc(text: &str) -> Result<Score> {
    let mut gregorian = if let Some((header_source, notation)) =
        scoretk_gabc_common::syntax::split_sections(text)
    {
        read::parse_payload(&header_source, &notation)?
    } else if scoretk_gabc_common::syntax::looks_like_gabc_body(text) {
        read::parse_body(text)?
    } else {
        return Err(Error::Syntax(
            "missing %% section separator (Gabc.tex:51,58)".to_owned(),
        ));
    };
    let mut score = Score::new();
    read::apply_metadata(&mut score, &gregorian);
    derived::build_projection(&mut score, &gregorian);
    gregorian.derived_modern_view = true;
    score.extensions.format = Some(scoretk_core::model::Extension::Gregorian(gregorian));
    Ok(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{Extension, Layers};

    const SIMPLE: &str = "name: Veni;\nlanguage: latin;\nmode: 2;\n%%\n(f3)\nVE(g.)ni(h') (,)\n";

    #[test]
    fn capabilities_declare_the_formats_layers() {
        assert_eq!(CAPABILITIES.writes, Layers::NONE);
        assert!(CAPABILITIES.reads.contains(Layers::EXTENSIONS));
        assert!(CAPABILITIES.reads.contains(Layers::TEXT));
        assert!(!CAPABILITIES.reads.contains(Layers::PLAYBACK));
        assert!(!CAPABILITIES.reads.contains(Layers::ENGRAVING));
    }

    #[test]
    fn reader_fills_both_the_primary_payload_and_the_derived_view() {
        let score = read_gabc(SIMPLE).expect("a valid GABC score");
        let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
            panic!("the L6 payload is present");
        };
        assert_eq!(gregorian.headers[0].name, "name");
        assert_eq!(gregorian.clefs, vec!["f3"]);
        assert_eq!(gregorian.syllables.len(), 3); // two syllables and a bar group
        assert_eq!(
            gregorian
                .syllables
                .iter()
                .map(|syllable| syllable.notes.len())
                .sum::<usize>(),
            2
        );
        assert!(gregorian.derived_modern_view);
        assert_eq!(score.notes.len(), 2);
        // La mesure dérivée s'achève sur la barre `,` : les deux syllabes
        // chantées et le groupe de barre ne forment qu'une mesure.
        assert_eq!(score.measures.len(), 1);
        assert!(score
            .report
            .losses
            .iter()
            .any(|loss| loss.item.contains("derived-modern-view")));
    }

    #[test]
    fn missing_separator_is_an_error() {
        assert!(read_gabc("name: Veni;\n(c3) Ve(g)\n").is_err());
    }
}
