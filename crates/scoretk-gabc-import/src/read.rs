// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `doc/Gabc.tex` et de `src/gabc/gabc-score-determination.l` de
// Gregorio (GPL-3.0-or-later), modifié (réécrit en Rust) en 2026.
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

//! Source-level GABC parsing (`Gabc.tex:36-343,403-906`).
//!
//! The lexer switches into its `notes` state at `(` and returns to `score`
//! at `)` (`gabc-score-determination.l:385-404`). This reader mirrors that
//! split so that spaces outside parentheses remain word separators
//! (`Gabc.tex:60-61,241-245`) and escaped parentheses (`$:`, `:247-250`) do
//! not open a note group.

use scoretk_core::model::{BarKind, Gregorian, GregorianNote, Score, Syllable};
use scoretk_gabc_common::syntax::{self, is_pitch};

use crate::Error;

/// Parses the two sections into the L6 payload.
pub(super) fn parse_payload(header_source: &str, notation: &str) -> Result<Gregorian, Error> {
    let headers = syntax::parse_headers(header_source);
    let staff_lines = headers
        .iter()
        .find(|header| header.name == "staff-lines")
        .and_then(|header| header.value.bytes().find(|byte| byte.is_ascii_digit()))
        .map_or(4, |digit| digit - b'0')
        .clamp(2, 5);
    let body = notation.split_once('\n').map_or("", |(_, body)| body);
    let (syllables, clefs) = parse_notation(body);
    Ok(Gregorian {
        headers,
        staff_lines,
        clefs,
        syllables,
        header_source: header_source.to_owned(),
        // The `%%` line is part of the notation half of `split_sections`, so
        // concatenating both halves reproduces the source byte for byte.
        notation: notation.to_owned(),
        derived_modern_view: false,
    })
}

/// Copies the interpreted headers into the common metadata (L0).
pub(super) fn apply_metadata(score: &mut Score, gregorian: &Gregorian) {
    for header in &gregorian.headers {
        match header.name.as_str() {
            "name" => {
                score.metadata.work_title = header.value.clone();
                score.metadata.movement_title = header.value.clone();
            }
            "author" => score.metadata.composer = header.value.clone(),
            "transcriber" => score.metadata.poet = header.value.clone(),
            "book" => score.metadata.source = header.value.clone(),
            "office-part" => score.metadata.subtitle = header.value.clone(),
            "gabc-copyright" | "score-copyright" => {
                score.metadata.copyright = header.value.clone();
            }
            _ => {}
        }
    }
}

/// Walks the notation section, alternating syllable text and note groups.
///
/// Returns the syllables and the clefs in source order. A group containing
/// only a clef is not a syllable, but the clef is still primary data
/// (`Gabc.tex:847-870`).
fn parse_notation(body: &str) -> (Vec<Syllable>, Vec<String>) {
    let chars: Vec<char> = body.chars().collect();
    let mut syllables: Vec<Syllable> = Vec::new();
    let mut clefs: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut code = String::new();
    let mut in_notes = false;
    let mut escaped = false;
    let mut index = 0;
    while index < chars.len() {
        let char = chars[index];
        if in_notes {
            if char == ')' {
                let (mut syllable, group_clefs) = parse_group(&code);
                clefs.extend(group_clefs);
                syllable.text = take_text(&mut text);
                syllable.rubric = rubric(&syllable.text);
                if !syllable.notes.is_empty()
                    || !syllable.bars.is_empty()
                    || !syllable.signs.is_empty()
                    || !syllable.text.is_empty()
                {
                    syllables.push(syllable);
                }
                code.clear();
                in_notes = false;
            } else {
                code.push(char);
            }
        } else if escaped {
            text.push(char);
            escaped = false;
        } else if char == '$' {
            // `$` escapes the next character (`Gabc.tex:247-250`).
            text.push(char);
            escaped = true;
        } else if char == '(' {
            // The text accumulated before `(` is this group's syllable. A
            // space before it ends the previous word, even when the syllable
            // itself is trimmed (`Gabc.tex:60-61,241-245`).
            let raw = std::mem::take(&mut text);
            if let Some(previous) = syllables.last_mut() {
                previous.space_after = text_has_space(&raw);
            }
            let pending = raw.trim().to_owned();
            code.clear();
            in_notes = true;
            // Kept until `)` closes the group; a single buffer handles
            // comments and escapes uniformly.
            text = pending;
        } else if char == '%' {
            // A comment runs to end of line and suppresses the end-of-line
            // space (`Gabc.tex:63-64`).
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            index += 1;
            continue;
        } else {
            text.push(char);
        }
        index += 1;
    }
    (syllables, clefs)
}

fn take_text(text: &mut String) -> String {
    std::mem::take(text).trim().to_owned()
}

fn text_has_space(text: &str) -> bool {
    text.chars().any(char::is_whitespace)
}

/// Extracts `<sp>…</sp>` / `<alt>…</alt>` markup (`Gabc.tex:297-309`).
fn rubric(text: &str) -> Option<String> {
    for tag in ["sp", "alt"] {
        let open = format!("<{tag}>");
        if let Some(start) = text.find(&open) {
            let close = format!("</{tag}>");
            let end = text[start..]
                .find(&close)
                .map(|offset| start + offset + close.len())
                .unwrap_or(text.len());
            return Some(text[start..end].to_owned());
        }
    }
    None
}

/// Parses one parenthesised note group into its syllable and clefs.
fn parse_group(raw_code: &str) -> (Syllable, Vec<String>) {
    let code = strip_attributes(raw_code);
    let chars: Vec<char> = code.chars().collect();
    let clefs = syntax::parse_clefs(&code);
    let mut syllable = Syllable::default();
    let mut index = 0;
    while index < chars.len() {
        let rest: String = chars[index..].iter().collect();
        if let Some((bar, used)) = BarKind::parse_prefix(&rest) {
            syllable.bars.push(bar);
            index += used;
            // Bars may carry a vertical episema or a brace (`Gabc.tex:823-830`).
            if matches!(chars.get(index), Some('\'' | '_')) {
                let raw = chars[index].to_string();
                syllable
                    .signs
                    .push(syntax::parse_signs(&raw).into_iter().next().unwrap());
                index += 1;
            }
            continue;
        }
        if let Some(used) = syntax::clef_token_len(&rest) {
            index += used;
            continue;
        }
        let char = chars[index];
        if matches!(char, 'z' | 'Z') {
            index += 1;
            if matches!(chars.get(index), Some('+' | '-' | '0')) {
                index += 1;
            }
            continue;
        }
        if matches!(char, '|' | '!' | '@' | '-' | '[' | ']') {
            // NABC cuts (grammar:870-907), zero-width spaces (`:684-686`),
            // fusion (`:627-634`) and shape hints (`:689-700`).
            index += 1;
            continue;
        }
        if char == '/' {
            index += 1;
            if matches!(chars.get(index), Some('0' | '!')) {
                index += 1;
            } else if chars.get(index) == Some(&'[') {
                while index < chars.len() && chars[index] != ']' {
                    index += 1;
                }
                index += 1;
            }
            continue;
        }
        if is_pitch(char) {
            let start = index;
            index += 1;
            while index < chars.len() {
                let rest: String = chars[index..].iter().collect();
                if is_pitch(chars[index])
                    || BarKind::parse_prefix(&rest).is_some()
                    || syntax::clef_token_len(&rest).is_some()
                    || matches!(chars[index], 'z' | 'Z' | '|')
                {
                    break;
                }
                index += 1;
            }
            let glyph: String = chars[start + 1..index].iter().collect();
            syllable.notes.push(GregorianNote {
                letter: char,
                signs: syntax::parse_signs(&glyph),
                glyph,
            });
            continue;
        }
        index += 1;
    }
    (syllable, clefs)
}

/// Removes bracketed attributes from a note group (`Gabc.tex:689-700,908-931,
/// 1093-1118`); the raw source remains in the L6 payload.
fn strip_attributes(code: &str) -> String {
    let mut result = String::with_capacity(code.len());
    let mut depth = 0;
    for char in code.chars() {
        match char {
            '[' => depth += 1,
            ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => result.push(char),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, SignKind};

    fn group(code: &str) -> Syllable {
        parse_group(code).0
    }

    #[test]
    fn pitch_letters_are_staff_positions_not_concert_notes() {
        let syllable = group("gahG0");
        let letters: Vec<char> = syllable.notes.iter().map(|note| note.letter).collect();
        assert_eq!(letters, ['g', 'a', 'h', 'G']);
        assert_eq!(syllable.notes[3].glyph, "0");
    }

    #[test]
    fn neume_shapes_keep_their_glyphs_and_signs() {
        let syllable = group("g~h_/'..");
        let signs: Vec<(&str, SignKind)> = syllable
            .notes
            .iter()
            .flat_map(|note| note.signs.iter().map(|sign| (sign.raw.as_str(), sign.kind)))
            .collect();
        assert_eq!(
            signs,
            [
                ("~", SignKind::Liquescentia),
                ("_", SignKind::HorizontalEpisema),
                ("'", SignKind::Ictus),
                ("..", SignKind::PunctumMora),
            ]
        );
    }

    #[test]
    fn bars_are_parsed_longest_first() {
        assert_eq!(group("(::)").bars, [BarKind::DivisioFinalis]);
        assert_eq!(
            group(";1(:?)").bars,
            [BarKind::Dominican(1), BarKind::DivisioMaiorDot]
        );
    }

    #[test]
    fn brackets_are_layout_data_not_notes() {
        let syllable = group("g[cs:c]h[shape:stroke]i");
        assert_eq!(syllable.notes.len(), 3);
        assert_eq!(syllable.notes[0].glyph, "");
    }

    #[test]
    fn custos_line_break_and_nabc_cut_are_not_notes() {
        assert!(group("z").notes.is_empty());
        assert!(group("Z-").notes.is_empty());
        assert_eq!(group("g+|h").notes.len(), 2);
    }

    #[test]
    fn clef_changes_in_bar_groups_are_recorded() {
        let parsed = parse_group("(:c3)g");
        assert_eq!(parsed.1, ["c3"]);
        assert_eq!(parsed.0.bars, [BarKind::DivisioMaior]);
        assert_eq!(parsed.0.notes.len(), 1);
    }

    #[test]
    fn word_spaces_and_escaped_parentheses_survive() {
        let source = "name: x;\n%%\n(c3) DE(g)us(h) in(g) $(a) (::)\n";
        let (header, notation) = syntax::split_sections(source).expect("sections");
        let gregorian = parse_payload(&header, &notation).expect("payload");
        assert_eq!(gregorian.syllables[0].text, "DE");
        assert!(!gregorian.syllables[0].space_after);
        assert!(gregorian.syllables[1].space_after);
        assert_eq!(gregorian.syllables[3].text, "$(a)");
    }
}
