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
//! The primary data of GABC is the L6 syllable/neume stream; durations, metre
//! and concert pitch in the pivot are a derived projection
//! (`docs/agents/model-modularisation.md` §7.1). The writer **serialises the
//! structured payload**: headers, clefs, syllables, note letters, neume
//! glyphs, rhythmic signs, bars and word spaces each come from their field
//! (`Gabc.tex:51-64,67-154,228-343,408-443,452-870`). It never keeps a copy
//! of the source text: a byte-identical round trip would compare the input
//! with itself and measure nothing (`docs/agents/gabc.md` §8).

use scoretk_core::model::{Capabilities, Extension, Gregorian, Layers, Score};
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

/// Writes a GABC file by serialising the L6 payload.
///
/// Every token is rebuilt from a structured field. The only strings copied
/// out of the parse are *atomic GABC tokens* that have no decomposed form in
/// the model: a sign's spelling (`NeumeSign::raw`, e.g. `.1`, `_502`, `r3`),
/// a neume glyph (the spelling of one neume, `GregorianNote::glyph`) and a
/// clef token (`c3`, `cb3`, `c1@c4`). Each is justified in
/// `docs/agents/gabc.md` §8; everything else — headers, separators, bars,
/// word spaces, the `%%` marker — is generated here.
pub fn write_gabc(score: &Score) -> Result<String> {
    let Some(Extension::Gregorian(gregorian)) = &score.extensions.format else {
        return Err(Error::MissingGregorian);
    };
    Ok(emit(gregorian))
}

/// Serialises one payload. Kept separate from [`write_gabc`] so a score built
/// entirely from parsed fields can be tested without any source text.
fn emit(gregorian: &Gregorian) -> String {
    let mut out = String::with_capacity(256);
    if !gregorian.headers.iter().any(|header| header.name == "name") {
        // `name:` is mandatory (`Gabc.tex:79-82`); a payload assembled by
        // another direction may not have one.
        out.push_str("name: ;\n");
    }
    for header in &gregorian.headers {
        out.push_str(&header.name);
        out.push_str(": ");
        let mut lines = header.value.split('\n');
        let first = lines.next().unwrap_or("");
        out.push_str(first);
        let rest: Vec<&str> = lines.collect();
        if rest.is_empty() {
            out.push_str(";\n");
        } else {
            // Multi-line values omit the first `;` and end in `;;`
            // (`Gabc.tex:69-74`).
            out.push('\n');
            for (index, line) in rest.iter().enumerate() {
                out.push_str(line);
                out.push_str(if index + 1 == rest.len() {
                    ";;\n"
                } else {
                    "\n"
                });
            }
        }
    }
    out.push_str("%%\n\n");

    // A leading group holding only the initial clef is not a syllable
    // (§7 of the reader); it is emitted here from the source-order list.
    // A clef change carried by a syllable is emitted on that syllable, so it
    // stays where Gregorio applied it (`Gabc.tex:847-870`).
    if !gregorian
        .syllables
        .first()
        .is_some_and(|syllable| syllable.clef.is_some())
    {
        if let Some(clef) = gregorian.clefs.first() {
            out.push('(');
            out.push_str(clef);
            out.push_str(")\n");
        }
    }

    for syllable in &gregorian.syllables {
        out.push_str(&syllable.text);
        out.push('(');
        if let Some(clef) = &syllable.clef {
            out.push_str(clef);
        }
        for note in &syllable.notes {
            out.push(note.letter);
            if !note.glyph.is_empty() {
                out.push_str(&note.glyph);
            } else {
                // Signs stored beside the note (e.g. on a score assembled
                // from another format) use their atomic GABC spelling.
                for sign in &note.signs {
                    out.push_str(&sign.raw);
                }
            }
        }
        for bar in &syllable.bars {
            out.push_str(bar.as_str());
        }
        for sign in &syllable.signs {
            out.push_str(&sign.raw);
        }
        out.push(')');
        // Spaces and line ends are word separators and are significant
        // (`Gabc.tex:60-61,241-245`): the parsed flag decides which one.
        out.push(if syllable.space_after { ' ' } else { '\n' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, GregorianHeader, GregorianNote, NeumeSign, SignKind};

    fn header(name: &str, value: &str) -> GregorianHeader {
        GregorianHeader {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn capabilities_are_the_mirror_of_the_reader() {
        assert_eq!(CAPABILITIES.reads, Layers::NONE);
        assert!(CAPABILITIES.writes.contains(Layers::EXTENSIONS));
        assert!(!CAPABILITIES.writes.contains(Layers::PLAYBACK));
    }

    #[test]
    fn payload_has_no_raw_source_field() {
        // Compile-time proof: a writer reading raw text cannot compile once
        // the fields are gone. This source check keeps a future reintroduction
        // from silently restoring the self-comparison.
        let model = include_str!("../../scoretk-core/src/model/gregorian.rs");
        assert!(
            !model.contains("header_source"),
            "raw header source in the model"
        );
        assert!(
            !model.contains("pub notation:"),
            "raw notation in the model"
        );
    }

    #[test]
    fn every_token_is_rebuilt_from_a_structured_field() {
        let gregorian = Gregorian {
            headers: vec![
                header("name", "Veni"),
                header("language", "latin"),
                header("book", "Graduale\nRomanum"),
            ],
            staff_lines: 4,
            clefs: vec!["c3".to_owned()],
            syllables: vec![
                scoretk_core::model::Syllable {
                    text: "VE".to_owned(),
                    clef: None,
                    rubric: None,
                    notes: vec![
                        GregorianNote {
                            letter: 'g',
                            glyph: ".".to_owned(),
                            signs: vec![NeumeSign::new(SignKind::PunctumMora, ".")],
                        },
                        GregorianNote {
                            letter: 'h',
                            glyph: String::new(),
                            signs: vec![NeumeSign::new(SignKind::Ictus, "'")],
                        },
                    ],
                    signs: Vec::new(),
                    bars: Vec::new(),
                    space_after: true,
                },
                scoretk_core::model::Syllable {
                    text: "ni".to_owned(),
                    clef: None,
                    rubric: None,
                    notes: vec![GregorianNote {
                        letter: 'g',
                        glyph: String::new(),
                        signs: Vec::new(),
                    }],
                    signs: Vec::new(),
                    bars: vec![BarKind::DivisioFinalis],
                    space_after: false,
                },
            ],
            derived_modern_view: false,
        };
        let mut score = Score::new();
        score.extensions.format = Some(Extension::Gregorian(gregorian));
        assert_eq!(
            write_gabc(&score).expect("GABC output"),
            "name: Veni;\nlanguage: latin;\nbook: Graduale\nRomanum;;\n\
             %%\n\n(c3)\nVE(g.h') ni(g::)\n"
        );
    }

    #[test]
    fn a_clef_change_stays_on_its_own_syllable() {
        let mut gregorian = Gregorian {
            headers: vec![header("name", "Factus")],
            clefs: vec!["c4".to_owned(), "c3".to_owned()],
            ..Gregorian::default()
        };
        gregorian.syllables.push(scoretk_core::model::Syllable {
            text: " DE".to_owned(),
            clef: Some("c4".to_owned()),
            notes: vec![GregorianNote {
                letter: 'g',
                ..GregorianNote::default()
            }],
            ..scoretk_core::model::Syllable::default()
        });
        gregorian.syllables.push(scoretk_core::model::Syllable {
            text: " us".to_owned(),
            clef: Some("c3".to_owned()),
            notes: vec![GregorianNote {
                letter: 'g',
                ..GregorianNote::default()
            }],
            bars: vec![BarKind::DivisioFinalis],
            ..scoretk_core::model::Syllable::default()
        });
        assert_eq!(
            emit(&gregorian),
            "name: Factus;\n%%\n\n DE(c4g)\n us(c3g::)\n"
        );
    }
}
