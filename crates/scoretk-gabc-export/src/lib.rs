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

mod from_score;

use std::borrow::Cow;

use scoretk_core::model::{
    Capabilities, Extension, Gregorian, GregorianHeaderItem, Layers, Score,
};
use scoretk_gabc_common::Result;

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
/// word spaces, the `%%` marker — is generated here. A payload with no header
/// keeps its raw-body shape (no `name: ;`, no `%%`), as documented in §15/§17.
pub fn write_gabc(score: &Score) -> Result<String> {
    let gregorian: Cow<'_, Gregorian> = match &score.extensions.format {
        Some(Extension::Gregorian(gregorian)) => Cow::Borrowed(gregorian),
        // A score read from MusicXML, MuseScore or MIDI has no neume stream.
        // Rather than refusing it, build a plain-chant projection from its
        // concert pitches, lyrics and bar lines; `from_score` documents what
        // that projection cannot carry.
        // Other extension variants are payloads of another format: no GABC
        // neume stream either.
        _ => Cow::Owned(from_score::from_score(score)),
    };
    Ok(emit(&gregorian))
}

/// Serialises one payload. Kept separate from [`write_gabc`] so a score built
/// entirely from parsed fields can be tested without any source text.
fn emit(gregorian: &Gregorian) -> String {
    let mut out = String::with_capacity(256);
    // Comments, empty lines and field order are header items in their own
    // right (`Gabc.tex:51,63-64`; lexer `inicomments`,
    // `gabc-score-determination.l:129-133`). A payload assembled without a
    // source header falls back to the plain ordered field list.
    if gregorian.header_items.is_empty() && !gregorian.headers.is_empty() {
        if !gregorian.headers.iter().any(|header| header.name == "name") {
            // `name:` is mandatory (`Gabc.tex:79-82`); a payload assembled by
            // another direction may not have one.
            out.push_str("name: ;\n");
        }
        for header in &gregorian.headers {
            write_header_field(&mut out, header, false);
        }
    } else {
        for item in &gregorian.header_items {
            match item {
                GregorianHeaderItem::Comment(text) => {
                    out.push_str(text);
                    out.push('\n');
                }
                GregorianHeaderItem::Blank => out.push('\n'),
                GregorianHeaderItem::Field(index) => {
                    if let Some(header) = gregorian.headers.get(*index) {
                        let double = gregorian.header_double_semicolons.contains(index);
                        write_header_field(&mut out, header, double);
                    }
                }
            }
        }
    }
    if !gregorian.headers.is_empty() || !gregorian.header_items.is_empty() {
        out.push_str("%%\n");
        // Exact text between the marker and the first group; a file with a
        // header historically has one blank line there, a raw body none.
        if let Some(prefix) = &gregorian.notation_prefix {
            out.push_str(prefix);
        } else if !gregorian.headers.is_empty() {
            out.push('\n');
        }
    }

    if gregorian.headers.is_empty() && gregorian.header_items.is_empty() {
        // Raw GregoBase body: no header, no `%%` marker, but any leading
        // layout text (comments, indentation) is still source data.
        if let Some(prefix) = &gregorian.notation_prefix {
            out.push_str(prefix);
        }
    }

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
            out.push(')');
            // A raw body separates the clef group from the first syllable by
            // a space; a file with a header uses a line break. Both spellings
            // are part of the source layout and are kept verbatim (`§15/§17`).
            if let Some(separator) = &gregorian.opening_separator {
                out.push_str(separator);
            } else {
                out.push('\n');
            }
        }
    }

    for syllable in &gregorian.syllables {
        out.push_str(&syllable.text);
        out.push('(');
        if let Some(clef) = &syllable.clef {
            out.push_str(clef);
        }
        for attribute in &syllable.leading_attributes {
            out.push_str(attribute);
        }
        if let Some(above) = &syllable.above_text {
            // `Gabc.tex:1088-1091` : `[alt:…]` est un texte au-dessus de la
            // portée, émis à côté des notes mais jamais comme une note. Un
            // payload lu depuis du GABC porte déjà l'attribut à sa place
            // (dans `GregorianNote::attributes`) : ne pas le déplacer.
            let already_in_place = syllable.notes.iter().any(|note| {
                note.attributes
                    .iter()
                    .any(|attribute| attribute == &format!("[alt:{above}]"))
            });
            if !already_in_place {
                out.push_str("[alt:");
                out.push_str(above);
                out.push(']');
            }
        }
        // NABC snippets are atomic source tokens: the manuscript voice has
        // no decomposed GABC representation (spacing, modifiers, significant
        // letters). `@notes_before` keeps the square/manuscript
        // interleaving of the group, so the exact notation is reproduced.
        let mut note_index = 0;
        let mut after_snippet = false;
        for (snippet_position, snippet) in syllable.nabc.iter().enumerate() {
            // With `nabc-lines: 1`, a cut after a manuscript snippet returns
            // to square notation before the next square run. With more
            // manuscript voices, consecutive cuts stay in NABC state: adding
            // a cut-back would duplicate the separator (`||`) and lose the
            // voice index. Only emit it when square notes really separate
            // the two snippets (`gabc-score-determination.y:879,906`).
            if after_snippet && note_index < snippet.notes_before.min(syllable.notes.len()) {
                out.push('|');
                // The cut-back closes the previous manuscript snippet: emit
                // exactly what opened the square segment in the source (a
                // slash spacing glyph must not stay glued to the last square
                // note before the snippet, `docs/agents/gabc.md` §17).
                if let Some(previous) = syllable.nabc.get(snippet_position - 1) {
                    out.push_str(&previous.resume);
                }
            }
            while note_index < snippet.notes_before.min(syllable.notes.len()) {
                write_note(&mut out, &syllable.notes[note_index]);
                note_index += 1;
            }
            // Separation bars and group signs must stay in square state:
            // emitting them after the last `|` would make them part of the
            // manuscript snippet when the GABC is read back.
            if snippet_position + 1 == syllable.nabc.len() && note_index == syllable.notes.len() {
                write_bars(&mut out, syllable);
            }
            out.push('|');
            out.push_str(&snippet.text);
            after_snippet = true;
        }
        if after_snippet && note_index < syllable.notes.len() {
            out.push('|');
            if let Some(last) = syllable.nabc.last() {
                out.push_str(&last.resume);
            }
        }
        while note_index < syllable.notes.len() {
            write_note(&mut out, &syllable.notes[note_index]);
            note_index += 1;
        }
        if !after_snippet {
            write_bars(&mut out, syllable);
        }
        out.push(')');
        // Un saut de ligne est un séparateur de mot au même titre qu'un
        // espace (`Gabc.tex:60-61,241-245`) : en insérer un ici changerait
        // le `<syllabic>` dérivé au prochain aller-retour. Sans marqueur de
        // fin de mot, la syllabe suivante suit immédiatement.
        if let Some(separator) = &syllable.separator_after {
            out.push_str(separator);
        } else if syllable.space_after {
            out.push(' ');
        }
    }
    out
}

/// Serialises one header field. `double` keeps a `;;` terminator even when
/// the value has no continuation line (`Gabc.tex:69-74`).
fn write_header_field(
    out: &mut String,
    header: &scoretk_core::model::GregorianHeader,
    double: bool,
) {
    out.push_str(&header.name);
    out.push_str(": ");
    let mut lines = header.value.split('\n');
    let first = lines.next().unwrap_or("");
    out.push_str(first);
    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() {
        out.push_str(if double { ";;\n" } else { ";\n" });
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

/// Barres de séparation et signes de groupe, émis en état carré.
fn write_bars(out: &mut String, syllable: &scoretk_core::model::Syllable) {
    for bar in &syllable.bars {
        out.push_str(bar.as_str());
    }
    for sign in &syllable.signs {
        out.push_str(&sign.raw);
    }
}

/// Sérialise une lettre chantée et son glyphe atomique (`gabc.md` §8).
fn write_note(out: &mut String, note: &scoretk_core::model::GregorianNote) {
    out.push(note.letter);
    if !note.glyph.is_empty() {
        out.push_str(&note.glyph);
    } else {
        // Signs stored beside the note (e.g. on a score assembled from
        // another format) use their atomic GABC spelling.
        for sign in &note.signs {
            out.push_str(&sign.raw);
        }
    }
    // Layout/tuning/TeX attributes are atomic GABC tokens attached to this
    // glyph (`Gabc.tex:689-700,908-931,1093-1118`), emitted in source order.
    for attribute in &note.attributes {
        out.push_str(attribute);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{
        BarKind, GregorianHeader, GregorianNote, NabcSnippet, NeumeSign, SignKind,
    };

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
            opening_separator: None,
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
                            ..GregorianNote::default()
                        },
                        GregorianNote {
                            letter: 'h',
                            glyph: String::new(),
                            signs: vec![NeumeSign::new(SignKind::Ictus, "'")],
                            ..GregorianNote::default()
                        },
                    ],
                    signs: Vec::new(),
                    bars: Vec::new(),
                    space_after: true,
                    ..scoretk_core::model::Syllable::default()
                },
                scoretk_core::model::Syllable {
                    text: "ni".to_owned(),
                    clef: None,
                    rubric: None,
                    notes: vec![GregorianNote {
                        letter: 'g',
                        glyph: String::new(),
                        signs: Vec::new(),
                        ..GregorianNote::default()
                    }],
                    signs: Vec::new(),
                    bars: vec![BarKind::DivisioFinalis],
                    space_after: false,
                    ..scoretk_core::model::Syllable::default()
                },
            ],
            derived_modern_view: false,
            ..Gregorian::default()
        };
        let mut score = Score::new();
        score.extensions.format = Some(Extension::Gregorian(gregorian));
        assert_eq!(
            write_gabc(&score).expect("GABC output"),
            "name: Veni;\nlanguage: latin;\nbook: Graduale\nRomanum;;\n\
             %%\n\n(c3)\nVE(g.h') ni(g::)"
        );
    }

    #[test]
    fn a_headerless_body_is_written_without_inventing_a_header() {
        let gregorian = Gregorian {
            headers: Vec::new(),
            clefs: vec!["c4".to_owned()],
            syllables: vec![scoretk_core::model::Syllable {
                text: " Ky".to_owned(),
                notes: vec![GregorianNote {
                    letter: 'f',
                    ..GregorianNote::default()
                }],
                space_after: true,
                ..scoretk_core::model::Syllable::default()
            }],
            ..Gregorian::default()
        };
        assert_eq!(emit(&gregorian), "(c4)\n Ky(f) ");
    }

    #[test]
    fn consecutive_manuscript_voices_keep_a_single_cut_each() {
        // With seven declared voices, consecutive cuts move from NABC voice
        // 1 to 7, not back to square notation: a cut-back would emit `||`
        // (`gabc-score-determination.y:879,906`).
        let mut gregorian = Gregorian {
            headers: vec![header("nabc-lines", "7")],
            clefs: vec!["f3".to_owned()],
            ..Gregorian::default()
        };
        gregorian.syllables.push(scoretk_core::model::Syllable {
            notes: vec![GregorianNote {
                letter: 'i',
                ..GregorianNote::default()
            }],
            nabc: ["bv-", "gh", "to"]
                .iter()
                .map(|text| scoretk_core::model::NabcSnippet {
                    notes_before: 1,
                    text: (*text).to_owned(),
                    ..NabcSnippet::default()
                })
                .collect(),
            ..scoretk_core::model::Syllable::default()
        });
        assert_eq!(
            emit(&gregorian),
            "name: ;\nnabc-lines: 7;\n%%\n\n(f3)\n(i|bv-|gh|to)"
        );
    }

    #[test]
    fn a_slash_opening_a_square_snippet_follows_the_cut_back() {
        let gregorian = Gregorian {
            headers: vec![header("nabc-lines", "1")],
            clefs: vec!["c4".to_owned()],
            opening_separator: Some(" ".to_owned()),
            syllables: vec![scoretk_core::model::Syllable {
                text: "nos,".to_owned(),
                notes: ['h', 'h', 'h', 'g']
                    .into_iter()
                    .map(|letter| GregorianNote {
                        letter,
                        ..GregorianNote::default()
                    })
                    .collect(),
                nabc: vec![
                    NabcSnippet {
                        notes_before: 2,
                        text: "cl".to_owned(),
                        ..NabcSnippet::default()
                    },
                    NabcSnippet {
                        notes_before: 3,
                        text: "ds-".to_owned(),
                        resume: "/".to_owned(),
                    },
                ],
                separator_after: Some("\n".to_owned()),
                ..scoretk_core::model::Syllable::default()
            }],
            ..Gregorian::default()
        };
        assert_eq!(
            emit(&gregorian),
            "name: ;\nnabc-lines: 1;\n%%\n\n(c4) nos,(hh|cl|h|ds-|/g)\n"
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
        assert_eq!(emit(&gregorian), "name: Factus;\n%%\n\n DE(c4g) us(c3g::)");
    }
}
