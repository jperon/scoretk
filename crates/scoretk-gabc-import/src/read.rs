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

use scoretk_core::model::{
    BarKind, Gregorian, GregorianNote, GroupToken, GroupTokenKind, NabcSnippet, NoteAttribute,
    Score, Syllable,
};
use scoretk_gabc_common::syntax::{self, is_pitch};

use crate::Error;

/// Parses the two sections into the L6 payload.
pub(super) fn parse_payload(header_source: &str, notation: &str) -> Result<Gregorian, Error> {
    // Position source des notes : unités UTF-16 depuis le début du fichier
    // entier (indices de chaîne JavaScript). Le `%%` et sa fin de ligne
    // appartiennent à l'en-tête pour ce compte, comme le veut le consommateur.
    let marker_line = notation.find('\n').map_or(notation.len(), |end| end + 1);
    let base =
        header_source.encode_utf16().count() + notation[..marker_line].encode_utf16().count();
    parse_parts(header_source, &notation[marker_line..], base)
}

/// Parses a headerless body (raw GregoBase export) into the L6 payload.
///
/// The whole text is notation, so note offsets start at zero. The empty
/// header yields the default four-line staff (`Gabc.tex:128,434`).
pub(super) fn parse_body(body: &str) -> Result<Gregorian, Error> {
    parse_parts("", body, 0)
}

fn parse_parts(header_source: &str, body: &str, base: usize) -> Result<Gregorian, Error> {
    let (headers, header_items, header_double_semicolons) =
        syntax::parse_header_layout(header_source);
    let staff_lines = headers
        .iter()
        .find(|header| header.name == "staff-lines")
        .and_then(|header| header.value.bytes().find(|byte| byte.is_ascii_digit()))
        .map_or(4, |digit| digit - b'0')
        .clamp(2, 5);
    // `nabc-lines` (`:129-130`) gives the number of manuscript snippets that
    // may follow a square snippet; the `|` cut cycles the state modulo
    // `nabc_lines + 1` (`gabc-score-determination.y:870-907`). Without the
    // header, a raw GregoBase export still carries the cuts: GregoBase itself
    // prepends `nabc-lines: 1;` before rendering
    // (`specs/gabc/gregobase/chant_img.php:48`) and Gregorio documents the
    // pattern in `doc/GregorioNabcRef.tex:153-170`. Gregorio rejects `|` when
    // the count is 0 (`gabc-score-determination.y:870-907`, VERBOSITY_FATAL),
    // so infer that single voice when a cut is present, whatever the number
    // of cuts in one group. An explicit header always wins, including
    // `nabc-lines: 0;`.
    let nabc_lines = match headers.iter().find(|header| header.name == "nabc-lines") {
        Some(header) => header
            .value
            .bytes()
            .find(|byte| byte.is_ascii_digit())
            .map_or(0, |digit| (digit - b'0') as usize),
        None => crate::nabc::detect_nabc_lines(body),
    };
    let (syllables, clefs, opening_separator, notation_prefix) =
        parse_notation(body, base, nabc_lines, !headers.is_empty());
    Ok(Gregorian {
        headers,
        staff_lines,
        clefs,
        syllables,
        opening_separator,
        header_items,
        header_double_semicolons,
        notation_prefix,
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
/// Returns the syllables, the clefs in source order and the exact text after
/// an opening clef-only group. A group containing only a clef is not a
/// syllable, but the clef is still primary data (`Gabc.tex:847-870`).
fn parse_notation(
    body: &str,
    base: usize,
    nabc_lines: usize,
    has_header: bool,
) -> (Vec<Syllable>, Vec<String>, Option<String>, Option<String>) {
    let chars: Vec<char> = body.chars().collect();
    // `offsets[i]` est la position UTF-16 du caractère `chars[i]` dans le
    // fichier entier ; le dernier élément est la fin du corps.
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    let mut cursor = base;
    for char in &chars {
        offsets.push(cursor);
        cursor += char.len_utf16();
    }
    offsets.push(cursor);
    let mut syllables: Vec<Syllable> = Vec::new();
    let mut clefs: Vec<String> = Vec::new();
    let mut text = String::new();
    // Verbatim text between the previous `)` and the next `(`: whitespace
    // and whole-line `%` comments (`Gabc.tex:60-64`). The GABC writer emits
    // it back; the derived view keeps using `space_after`, so no other
    // direction is affected by the exact spelling.
    let mut separator = String::new();
    // Whitespace outside a comment: a `%` suppresses the end-of-line space
    // (`Gabc.tex:63-64`), so comment text must not set `space_after`.
    let mut separator_space = false;
    let mut notation_prefix: Option<String> = None;
    let mut pending_text_spacing: Option<String> = None;
    let mut code = String::new();
    let mut code_start = 0usize;
    let mut in_notes = false;
    let mut escaped = false;
    let mut seen_content = false;
    let mut seen_group = false;
    let mut opening_clef_seen = false;
    let mut opening_separator: Option<String> = None;
    let mut index = 0;
    while index < chars.len() {
        let char = chars[index];
        // A file with a header has no syllable before its first group: any
        // text there (a nested GregoBase header, a `%%` line) is layout.
        // Once a group has been read, text belongs to a syllable even when
        // that first group was layout rather than a recognised clef (`(v3)`
        // in the verbatim-TeX corpus): a `<v>(</v>` there is lyric text, not
        // a second group opener.
        let prefix_mode = has_header && !seen_group && text.is_empty();
        if in_notes {
            if char == ')' {
                let (mut syllable, group_clefs, above_text) =
                    parse_group(&code, &offsets[code_start..=index], nabc_lines);
                clefs.extend(group_clefs.iter().cloned());
                // The clef is stored on the group that carries it: an
                // in-score clef change must not be moved to the beginning by
                // the writer (`Gabc.tex:847-870`, `struct.c:1273-1288`).
                syllable.clef = group_clefs.into_iter().next();
                syllable.text = take_text(&mut text);
                syllable.text_spacing = pending_text_spacing.take();
                syllable.rubric = rubric(&syllable.text);
                syllable.above_text = above_text;
                let has_content = !syllable.notes.is_empty()
                    || !syllable.bars.is_empty()
                    || !syllable.signs.is_empty()
                    || !syllable.text.is_empty();
                // A leading group holding only the initial clef is recorded
                // in `Gregorian::clefs` and emitted before the first
                // syllable; a later clef-only group needs its own syllable.
                let initial_clef_group = !has_content
                    && syllable.clef.is_some()
                    && syllables.is_empty()
                    && !opening_clef_seen;
                if has_content || (syllable.clef.is_some() && seen_content) {
                    syllables.push(syllable);
                } else {
                    // An empty group (`()`, `( )`, a second initial clef, a
                    // slash-only snippet) is layout, not a sung event. Its
                    // exact parentheses are kept in the inter-group layout
                    // field so the writer can reproduce it, while the
                    // derived view stays untouched.
                    if !initial_clef_group {
                        separator.push('(');
                        separator.push_str(&code);
                        separator.push(')');
                    } else {
                        opening_clef_seen = true;
                    }
                }
                if has_content {
                    seen_content = true;
                }
                code.clear();
                in_notes = false;
            } else {
                code.push(char);
            }
        } else if escaped {
            if prefix_mode {
                separator.push(char);
            } else {
                text.push(char);
            }
            escaped = false;
        } else if char == '$' {
            // `$` escapes the next character (`Gabc.tex:247-250`).
            if prefix_mode {
                separator.push(char);
            } else {
                text.push(char);
            }
            escaped = true;
        } else if char == '(' {
            // The text accumulated before `(` is this group's syllable. The
            // text between groups is kept verbatim for the GABC writer, while
            // `space_after` still records only the *fact* of a word break for
            // the derived view (`Gabc.tex:60-61,241-245`).
            // `space_after` keeps the historical rule (`text_has_space` on
            // everything accumulated since the previous group), so the
            // derived view and the five other destinations are unchanged.
            // Only the exact spelling is newly stored in `separator_after`.
            let space_after = separator_space || text.chars().any(char::is_whitespace);
            let buffered = std::mem::take(&mut text);
            // Whitespace on either side of the syllable text is inter-group
            // layout, not part of the syllable (`Gabc.tex:60-61`). Markup
            // such as `</eu> (::)` ends in a space, so `trim_start` alone
            // would lose it.
            // Leading whitespace belongs to the previous group (word break);
            // trailing whitespace is the exact gap before this group's `(`.
            // It is a local layout field, not part of the lyric text.
            let trimmed = buffered.trim();
            let lead = &buffered[..buffered.len() - buffered.trim_start().len()];
            pending_text_spacing = if trimmed.is_empty() {
                None
            } else {
                Some(buffered[buffered.trim_end().len()..].to_owned())
            };
            let raw_separator = {
                let mut raw = std::mem::take(&mut separator);
                raw.push_str(lead);
                raw
            };
            let pending = trimmed.to_owned();
            if let Some(previous) = syllables.last_mut() {
                previous.space_after = space_after;
                // Several empty groups may occur before the next syllable:
                // their parentheses are layout tokens in the same field.
                previous
                    .separator_after
                    .get_or_insert_with(String::new)
                    .push_str(&raw_separator);
            } else if opening_clef_seen {
                opening_separator
                    .get_or_insert_with(String::new)
                    .push_str(&raw_separator);
            } else {
                // Everything before the first group, including a nested
                // header block or a `%` comment, is source layout.
                notation_prefix
                    .get_or_insert_with(String::new)
                    .push_str(&raw_separator);
            }
            separator_space = false;
            code.clear();
            code_start = index + 1;
            in_notes = true;
            seen_group = true;
            text = pending;
        } else if char == '<' && chars[index..].iter().collect::<String>().starts_with("<v>") {
            // Verbatim TeX `<v>…</v>` is typography, not a note group: a
            // literal `(` inside it must not open a group
            // (`Gabc.tex:297-309,1093-1102`, pitfall 9 of format-ref). Keep
            // the whole tag as syllable text so the writer emits it back.
            let rest: String = chars[index..].iter().collect();
            if let Some(end) = rest.find("</v>") {
                let end = index + end + 4;
                if prefix_mode {
                    separator.extend(&chars[index..end]);
                } else {
                    text.push_str(&chars[index..end].iter().collect::<String>());
                }
                index = end;
                continue;
            }
            if prefix_mode {
                separator.push(char);
            } else {
                text.push(char);
            }
        } else if char == '%' {
            // A comment runs to end of line and suppresses the end-of-line
            // space (`Gabc.tex:63-64`). Its exact characters are layout, so
            // they are kept in the inter-group separator, never in the
            // syllable text (and therefore never in a derived lyric).
            let start = index;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            if index < chars.len() {
                index += 1;
            }
            // Whole-line comments occur between groups in the corpus; they
            // belong to the exact separator, never to a derived lyric.
            separator.extend(&chars[start..index]);
            continue;
        } else if char.is_whitespace() && text.is_empty() {
            separator.push(char);
            separator_space = true;
        } else {
            if prefix_mode {
                // A second, GregoBase-style header block may follow the
                // first `%%` (`name: …;` lines): it is notation layout for
                // scoretk, not syllable text (the Gabc.tex header grammar
                // `:67-74` and the `%%` rule `:51,58`).
                let end = chars[index..]
                    .iter()
                    .position(|c| matches!(c, '\n' | '('))
                    .map_or(chars.len(), |offset| index + offset);
                let line: String = chars[index..end].iter().collect();
                if line.split_once(':').is_some_and(|(name, _)| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                }) {
                    separator.extend(&chars[index..end]);
                    index = end;
                    continue;
                }
            }
            text.push(char);
        }
        index += 1;
    }
    // The text after the last `)` is still part of the notation's layout: a
    // raw GregoBase body ends with a newline that a byte-exact writer must
    // reproduce (`Gabc.tex:60-61,241-245`).
    if let Some(last) = syllables.last_mut() {
        last.separator_after
            .get_or_insert_with(String::new)
            .push_str(&separator);
    }
    (syllables, clefs, opening_separator, notation_prefix)
}

fn take_text(text: &mut String) -> String {
    std::mem::take(text).trim().to_owned()
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
///
/// `offsets` gives the UTF-16 position of each character of `raw_code` (plus
/// a final sentinel) so every note keeps its exact source location even
/// when bracketed layout attributes are stripped.
fn parse_group(
    raw_code: &str,
    offsets: &[usize],
    nabc_lines: usize,
) -> (Syllable, Vec<String>, Option<String>) {
    let above_text = above_staff_text(raw_code);
    let mut syllable = Syllable {
        above_text: above_text.clone(),
        ..Syllable::default()
    };
    let (code, mapping, brackets) = strip_attributes(raw_code);
    // A `|` cut cycles the NABC state modulo `nabc_lines + 1`
    // (`gabc-score-determination.y:870-907`): state 0 is square notation,
    // states 1…n are the parallel manuscript snippets. NABC letters (`a`…`n`)
    // are staff positions of a *different* notation and must never be parsed
    // as square pitches or signs (`GregorioNabcRef.tex:178-199`). Square
    // snippets after a later cut are still sung notes and are parsed too.
    let mut chars = Vec::new();
    let mut source = Vec::new();
    // Split the group into square (`false`) and manuscript (`true`) segments
    // before parsing notes. Keeping the segments separate is what allows a
    // slash opening a square segment after a cut to stay on that side of the
    // cut instead of being swallowed by the previous note's glyph
    // (`docs/agents/gabc.md` §17).
    let mut segments: Vec<(bool, Vec<(char, usize)>)> = vec![(false, Vec::new())];
    let mut state = 0usize;
    for (char, &raw_position) in code.chars().zip(mapping.iter()) {
        if char == '|' {
            state = (state + 1) % (nabc_lines.max(1) + 1);
            segments.push((state != 0, Vec::new()));
        } else {
            segments
                .last_mut()
                .expect("a notation segment")
                .1
                .push((char, raw_position));
        }
    }
    // A slash at the start of a square segment is the spacing glyph that
    // opens it; detach it from that segment and attach it to the manuscript
    // snippet it follows, so the writer places it after the cut-back.
    let mut resumes = vec![String::new(); segments.len()];
    let mut detached = vec![0usize; segments.len()];
    for index in 0..segments.len().saturating_sub(1) {
        if !segments[index].0 || segments[index + 1].0 {
            continue;
        }
        let mut resume = String::new();
        for (char, _) in &segments[index + 1].1 {
            if *char == '/' || (!resume.is_empty() && matches!(char, '0' | '!')) {
                resume.push(*char);
            } else {
                break;
            }
        }
        detached[index + 1] = resume.chars().count();
        resumes[index] = resume;
    }
    // (index in the square character stream at the cut, exact snippet text,
    // characters opening the square snippet after the cut-back).
    let mut cuts: Vec<(usize, String, String)> = Vec::new();
    for (index, (is_nabc, content)) in segments.into_iter().enumerate() {
        if is_nabc {
            let text: String = content.iter().map(|(char, _)| *char).collect();
            if !text.is_empty() {
                cuts.push((chars.len(), text, resumes[index].clone()));
            }
        } else {
            for (char, position) in content.into_iter().skip(detached[index]) {
                chars.push(char);
                source.push(offsets[position]);
            }
        }
    }
    let clefs = syntax::parse_clefs(&chars.iter().collect::<String>());
    let square_len = chars.len();
    let mut note_starts: Vec<usize> = Vec::new();
    let mut note_ends: Vec<usize> = Vec::new();
    let mut group_tokens: Vec<GroupToken> = Vec::new();
    // Whitespace following a non-note token belongs to that token's layout:
    // notes keep their own glyph spacing (`Gabc.tex:60-61`).
    let trailing_ws = |index: &mut usize| {
        while *index < square_len && chars[*index].is_whitespace() {
            *index += 1;
        }
    };
    let mut index = 0;
    while index < square_len {
        let start = index;
        if index == 0 && chars[index].is_whitespace() {
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            group_tokens.push(GroupToken {
                notes_before: 0,
                text: chars[start..index].iter().collect(),
                kind: GroupTokenKind::Spacing,
            });
            continue;
        }
        let rest: String = chars[index..].iter().collect();
        if let Some((bar, used)) = BarKind::parse_prefix(&rest) {
            syllable.bars.push(bar);
            index += used;
            // Bars may carry a vertical episema or a brace (`Gabc.tex:823-830`).
            if matches!(chars.get(index), Some('\'' | '_')) {
                index += 1;
            }
            trailing_ws(&mut index);
            group_tokens.push(GroupToken {
                notes_before: note_starts.len(),
                text: chars[start..index].iter().collect(),
                kind: GroupTokenKind::Bar,
            });
            continue;
        }
        if let Some(used) = syntax::clef_token_len(&rest) {
            index += used;
            trailing_ws(&mut index);
            group_tokens.push(GroupToken {
                notes_before: note_starts.len(),
                text: chars[start..index].iter().collect(),
                kind: GroupTokenKind::Clef,
            });
            continue;
        }
        let char = chars[index];
        if matches!(char, 'z' | 'Z') {
            index += 1;
            if matches!(chars.get(index), Some('+' | '-' | '0')) {
                index += 1;
            }
            trailing_ws(&mut index);
            group_tokens.push(GroupToken {
                notes_before: note_starts.len(),
                text: chars[start..index].iter().collect(),
                kind: GroupTokenKind::Custos,
            });
            continue;
        }
        if matches!(char, '|' | '!' | '@' | '-' | '[' | ']') {
            // NABC cuts (grammar:870-907), zero-width spaces (`:684-686`),
            // fusion (`:627-634`) and shape hints (`:689-700`).
            index += 1;
            if char != '|' {
                trailing_ws(&mut index);
                group_tokens.push(GroupToken {
                    notes_before: note_starts.len(),
                    text: char.to_string(),
                    kind: GroupTokenKind::Spacing,
                });
            }
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
            trailing_ws(&mut index);
            group_tokens.push(GroupToken {
                notes_before: note_starts.len(),
                text: chars[start..index].iter().collect(),
                kind: GroupTokenKind::Spacing,
            });
            continue;
        }
        if is_pitch(char) && matches!(chars.get(index + 1), Some('+')) {
            // `[a-np]+` is a manual custos, not a note
            // (`Gabc.tex:872-884`; `gabc-notes-determination.l:1158-1162`).
            index += 2;
            trailing_ws(&mut index);
            group_tokens.push(GroupToken {
                notes_before: note_starts.len(),
                text: chars[start..index].iter().collect(),
                kind: GroupTokenKind::Custos,
            });
            continue;
        }
        if is_pitch(char) {
            let start = index;
            note_starts.push(start);
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
            note_ends.push(index);
            syllable.notes.push(GregorianNote {
                letter: char,
                signs: syntax::parse_signs(&glyph),
                glyph,
                source_offset: source.get(start).copied(),
                source_end: index.checked_sub(1).and_then(|last| {
                    source
                        .get(last)
                        .map(|start| start + chars[last].len_utf16())
                }),
                attributes: Vec::new(),
            });
            continue;
        }
        index += 1;
    }
    syllable.group_tokens = group_tokens;
    // Each bracketed attribute lives on the note whose token it follows,
    // not on the group: that is the level where Gregorio attaches it and
    // the level from which the GABC writer re-emits it in place.
    for (position, token) in brackets {
        let owner = note_starts
            .iter()
            .zip(&note_ends)
            .position(|(start, end)| position >= *start && position <= *end);
        match owner {
            Some(index) => syllable.notes[index].attributes.push(NoteAttribute {
                after: position.saturating_sub(note_starts[index] + 1),
                text: token,
            }),
            None => syllable.leading_attributes.push(token),
        }
    }
    syllable.nabc = cuts
        .into_iter()
        .map(|(cut, text, resume)| NabcSnippet {
            notes_before: note_starts.iter().filter(|&&start| start < cut).count(),
            text,
            resume,
        })
        .collect();
    (syllable, clefs, above_text)
}

/// Extrait `[alt:…]`, texte au-dessus de la portée (`Gabc.tex:1088-1091`).
///
/// Les autres crochets sont des attributs de mise en page ou du TeX brut
/// (`:689-700,908-931,1093-1118`) : ils ne sont pas du texte chanté et ne
/// sont pas non plus des notes.
fn above_staff_text(code: &str) -> Option<String> {
    let start = code.find("[alt:")? + "[alt:".len();
    let end = code[start..].find(']')? + start;
    Some(code[start..end].to_owned())
}

/// Removes bracketed layout/tuning attributes from a note group
/// (`Gabc.tex:689-700,908-931,1093-1118`), but **not** the pitch letters of a
/// fusion group introduced by `@[…]` (`:603-666`): there the brackets group
/// primitive notes that are all sung (`@[ehg]` = `e`, `h`, `g`), so stripping
/// them silently lost notes. The caller iterates the same characters and
/// treats the `@` as a fusion marker, leaving one note per letter.
fn strip_attributes(code: &str) -> (String, Vec<usize>, Vec<(usize, String)>) {
    let chars: Vec<char> = code.chars().collect();
    let mut result = String::with_capacity(code.len());
    // Position d'origine de chaque caractère conservé, plus un sentinelle.
    let mut mapping = Vec::with_capacity(chars.len() + 1);
    // Brackets kept as local tokens: their position in the stripped stream
    // says which note's glyph they belong to, their text is atomic because
    // Gregorio parses `[cs:t]` / `[oh:h]` as one attribute (`:908-931`).
    let mut brackets: Vec<(usize, String)> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            // An `@` immediately before `[` introduces a fusion sequence
            // (`Gabc.tex:603-666`): its pitch letters are sung and must stay.
            let fusion = index > 0 && chars[index - 1] == '@';
            let mut depth = 1;
            let mut end = index + 1;
            while end < chars.len() && depth > 0 {
                match chars[end] {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    _ => {}
                }
                end += 1;
            }
            if fusion {
                // `@[ehg]` fuses three *sung* pitches (`Gabc.tex:603-666`):
                // keep the primitives **and** their delimiters, since both
                // are source tokens. `parse_group` emits `@`, `[` and `]` as
                // spacing tokens around the notes.
                for (offset, char) in chars[index..end].iter().enumerate() {
                    result.push(*char);
                    mapping.push(index + offset);
                }
            } else {
                brackets.push((result.chars().count(), chars[index..end].iter().collect()));
            }
            index = end;
            continue;
        }
        result.push(chars[index]);
        mapping.push(index);
        index += 1;
    }
    mapping.push(chars.len());
    (result, mapping, brackets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, SignKind};

    fn group(code: &str) -> Syllable {
        let offsets: Vec<usize> = (0..=code.chars().count()).collect();
        parse_group(code, &offsets, 0).0
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
    fn fusion_groups_keep_every_primitive_note() {
        // `Gabc.tex:603-666`: `@[ehg]` fuses three *sung* pitches; it is not
        // a bracketed layout attribute. `gb18458` lost five notes this way.
        let syllable = group("@[ehg]");
        let letters: Vec<char> = syllable.notes.iter().map(|note| note.letter).collect();
        assert_eq!(letters, ['e', 'h', 'g']);
        let syllable = group("@[ig]");
        let letters: Vec<char> = syllable.notes.iter().map(|note| note.letter).collect();
        assert_eq!(letters, ['i', 'g']);
        // Ordinary `[…]` attributes are still layout, not notes.
        assert!(group("g[h390]a")
            .notes
            .iter()
            .all(|note| note.glyph.is_empty()));
    }

    #[test]
    fn custos_line_break_and_nabc_cut_are_not_notes() {
        assert!(group("z").notes.is_empty());
        assert!(group("Z-").notes.is_empty());
        assert!(group("g+").notes.is_empty());
        // With the cut in force, the text after `|` is a manuscript snippet,
        // never a square note (`gabc-score-determination.y:870-907`).
        let offsets: Vec<usize> = (0..="g+|h".chars().count() + 1).collect();
        let (syllable, _, _) = parse_group("g+|h", &offsets, 1);
        assert!(syllable.notes.is_empty());
        assert_eq!(syllable.nabc.len(), 1);
        assert_eq!(syllable.nabc[0].text, "h");
    }

    #[test]
    fn clef_changes_in_bar_groups_are_recorded() {
        let offsets: Vec<usize> = (0..="(:c3)g".chars().count() + 1).collect();
        let parsed = parse_group("(:c3)g", &offsets, 0);
        assert_eq!(parsed.1, ["c3"]);
        assert_eq!(parsed.0.bars, [BarKind::DivisioMaior]);
        assert_eq!(parsed.0.notes.len(), 1);
    }

    #[test]
    fn a_parenthesis_inside_verbatim_tex_is_not_a_note_group() {
        let source = "name: x;\n%%\n(v3) <v>(</v>Al(f)le(g) (::)\n";
        let (header, notation) = syntax::split_sections(source).expect("sections");
        let gregorian = parse_payload(&header, &notation).expect("payload");
        assert_eq!(gregorian.syllables[0].text, "<v>(</v>Al");
        assert_eq!(gregorian.syllables[0].notes.len(), 1);
        assert_eq!(gregorian.syllables[1].text, "le");
        assert_eq!(gregorian.syllables[1].notes.len(), 1);
    }

    #[test]
    fn above_staff_text_is_a_syllable_fact_not_a_note() {
        let syllable = group("g[alt:Fa]h");
        assert_eq!(syllable.above_text.as_deref(), Some("Fa"));
        assert_eq!(syllable.notes.len(), 2);
        assert!(group("g[cs:c]h").above_text.is_none());
    }

    #[test]
    fn utf16_source_offsets_point_at_the_sung_pitch_letter() {
        // Mesuré sur exsurge 2.8.7 par le consommateur Cantus. `é` vaut une
        // unité UTF-16 mais deux octets UTF-8 : les positions sont donc des
        // indices de chaîne JavaScript, pas des octets. Dans le corps, f=7,
        // g=8, h=9, le `g` de `gixi`=13 et le `i` chanté (après `x`)=16.
        let source = "name:t;\n%%\n(c4) é(fgh)b(gixi) (::)\n";
        let (header, notation) = syntax::split_sections(source).expect("sections");
        let gregorian = parse_payload(&header, &notation).expect("payload");
        let notes: Vec<&GregorianNote> = gregorian
            .syllables
            .iter()
            .flat_map(|syllable| syllable.notes.iter())
            .collect();
        // Le payload L6 garde le glyphe d'alteration (`i` à 25) pour un
        // aller-retour exact ; la projection n'en fait pas une note chantée.
        let offsets: Vec<usize> = notes
            .iter()
            .map(|note| note.source_offset.unwrap())
            .collect();
        assert_eq!(offsets, [18, 19, 20, 24, 25, 27]);
        assert_eq!(notes[4].glyph, "x");
        let ends: Vec<usize> = notes.iter().map(|note| note.source_end.unwrap()).collect();
        assert_eq!(ends, [19, 20, 21, 25, 27, 28]);
        let score = crate::read_gabc(source).expect("projected score");
        let projected: Vec<usize> = score
            .notes
            .iter()
            .map(|note| note.source.unwrap().offset)
            .collect();
        assert_eq!(projected, [18, 19, 20, 24, 27]);
        assert_eq!(source.encode_utf16().nth(27), Some('i' as u16));
    }

    #[test]
    fn body_layout_and_nabc_cut_slash_are_kept_in_the_payload() {
        // GregoBase 18205: the slash opens the square snippet after the `ds-`
        // manuscript snippet, and the raw body separates the clef and the
        // words with spaces (`docs/agents/gabc.md` §15/§17).
        const BODY: &str =
            "(c4) nos,(hg|cl|h|st|jj|ds-hi|jvHG|ci|hh|ds-|/giHG|pesu2|hf/fe|cl!pi) (:)\n";
        let payload = parse_body(BODY).expect("headerless body");
        assert_eq!(payload.opening_separator.as_deref(), Some(" "));
        assert_eq!(payload.syllables[0].separator_after.as_deref(), Some(" "));
        assert_eq!(payload.syllables[1].separator_after.as_deref(), Some("\n"));
        let snippets = &payload.syllables[0].nabc;
        assert_eq!(snippets.len(), 7);
        assert_eq!(snippets[4].text, "ds-");
        assert_eq!(snippets[4].resume, "/");
        assert_eq!(
            snippets
                .iter()
                .filter(|snippet| !snippet.resume.is_empty())
                .count(),
            1
        );
        // The second `h` of `hh` must not swallow the slash: its glyph is
        // empty and the slash is emitted with the cut-back on write.
        assert_eq!(payload.syllables[0].notes[9].glyph, "");
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
