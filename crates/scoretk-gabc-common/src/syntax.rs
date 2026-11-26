// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `doc/Gabc.tex` et `src/struct.{c,h}` de Gregorio (GPL-3.0-or-later),
// modifié (réécrit en Rust) en 2026. Each function cites the spec rule.
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

//! GABC syntax tables shared by the reader and the writer.
//!
//! [`staff_height`], [`clef_key`] and [`diatonic_step`] reproduce Gregorio's
//! own pitch resolution: staff heights start at `LOWEST_PITCH = 3`
//! (`src/struct.h:809`), the letter `o` is skipped (`src/dump/dump.c:78-88`)
//! and a clef's key is `2*line-1` for C, `2*line-4` for F
//! (`src/struct.c:1273-1288`). A letter is a typographical position, **not**
//! a pitch name (`doc/Gabc.tex:410-413`).

use scoretk_core::model::{GregorianHeader, GregorianHeaderItem, NeumeSign, SignKind};

/// True for a GABC pitch letter: `a`–`n` and `p`, with `o` reserved for the
/// oriscus (`Gabc.tex:410-413,469`; `src/dump/dump.c:78-88`).
pub fn is_pitch(letter: char) -> bool {
    matches!(letter, 'a'..='n' | 'A'..='N' | 'p' | 'P')
}

/// Staff height of a letter, starting at `a` = 3 (`src/struct.h:809-813`).
///
/// `o` is skipped, so `p` sits one step above `n` (`src/dump/dump.c:78-88`;
/// `Gabc.tex:443`).
pub fn staff_height(letter: char) -> i32 {
    let low = letter.to_ascii_lowercase();
    let base = low as i32 - 'a' as i32 + 3;
    if low > 'o' {
        base + 1
    } else {
        base
    }
}

/// Diatonic key of a clef token (`src/struct.c:1273-1288`).
///
/// `c3` → 5, `f3` → 2. For two simultaneous clefs (`c1@c4`,
/// `Gabc.tex:854-856`) the first one governs the pitch resolution.
pub fn clef_key(token: &str) -> i32 {
    let first = token.split('@').next().unwrap_or(token);
    let bytes = first.as_bytes();
    let line = bytes
        .iter()
        .rev()
        .find(|byte| byte.is_ascii_digit())
        .copied()
        .unwrap_or(b'4');
    let line = (line - b'0') as i32;
    if first.starts_with('f') {
        2 * line - 4
    } else {
        2 * line - 1
    }
}

/// Diatonic step (C = 0) of a letter under a clef.
///
/// The origin is arbitrary; only equality between two readings matters. The
/// formula and its verification against Gregorio's `initial_key 5 (c3)` are
/// recorded in `scripts/oracle/gabc/canonical.py`.
pub fn diatonic_step(letter: char, clef: &str) -> i32 {
    staff_height(letter) - clef_key(clef) - 5
}

/// Concert MIDI pitch of a derived note, with middle C = 60 for step 0.
///
/// This is a *derived* projection: GABC has no concert pitch and no duration
/// (`Gabc.tex:410-413,801-845`); the L6 payload remains the primary data.
pub fn concert_pitch(step: i32) -> u8 {
    const SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    let octave = step.div_euclid(7);
    (60 + SEMITONES[step.rem_euclid(7) as usize] + 12 * octave).clamp(0, 127) as u8
}

/// Tonal pitch class (C = 14, D = 16, …) of a derived diatonic step.
pub fn tpc_of_step(step: i32) -> i8 {
    const TPC: [i8; 7] = [14, 16, 18, 13, 15, 17, 19];
    TPC[step.rem_euclid(7) as usize]
}

/// Finds clef tokens in a note group (`Gabc.tex:847-870`).
///
/// A clef starts at the beginning of a group, after a bar or after whitespace;
/// a letter following another note letter is a glyph, not a clef. Two clefs
/// linked by `@` are one token (`:854-856`).
pub fn parse_clefs(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some((token, used)) = clef_at(&chars, index) {
            result.push(token);
            index += used;
        } else {
            index += 1;
        }
    }
    result
}

/// Length of a clef token starting at the beginning of `code`, if any.
///
/// Used by the reader to avoid treating a clef change embedded in a bar group
/// (`Gabc.tex:847-870`) as a note glyph.
pub fn clef_token_len(code: &str) -> Option<usize> {
    let chars: Vec<char> = code.chars().collect();
    clef_at(&chars, 0).map(|(_, used)| used)
}

fn clef_at(chars: &[char], at: usize) -> Option<(String, usize)> {
    if at > 0 && (chars[at - 1].is_ascii_alphanumeric() || chars[at - 1] == '+') {
        return None;
    }
    let first = one_clef(chars, at)?;
    let mut token = first.0.clone();
    let mut used = first.1;
    if at + used < chars.len() && chars[at + used] == '@' {
        if let Some(second) = one_clef(chars, at + used + 1) {
            token.push('@');
            token.push_str(&second.0);
            used += 1 + second.1;
        }
    }
    Some((token, used))
}

fn one_clef(chars: &[char], at: usize) -> Option<(String, usize)> {
    if !matches!(chars.get(at), Some('c' | 'f')) {
        return None;
    }
    let mut used = 1;
    if matches!(chars.get(at + used), Some('b')) {
        used += 1;
    }
    let line = *chars.get(at + used)?;
    if !('1'..='9').contains(&line) {
        return None;
    }
    used += 1;
    Some((chars[at..at + used].iter().collect(), used))
}

/// Parses the signs appended to one note glyph (`Gabc.tex:702-797`).
///
/// Shape families (oriscus, quilisma, virga, liquescents, …) are recorded
/// beside rhythmic signs (`:452-490`), with their exact source spelling, so a
/// conversion can report what it cannot carry.
pub fn parse_signs(glyph: &str) -> Vec<NeumeSign> {
    let chars: Vec<char> = glyph.chars().collect();
    let mut signs = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let char = chars[index];
        let mut end = index + 1;
        let kind = match char {
            '.' => {
                while end < chars.len() && end - index < 3 && matches!(chars[end], '.' | '0' | '1')
                {
                    end += 1;
                }
                Some(SignKind::PunctumMora)
            }
            '\'' => {
                if matches!(chars.get(end), Some('0' | '1')) {
                    end += 1;
                }
                Some(SignKind::Ictus)
            }
            '_' => {
                while matches!(chars.get(end), Some('0'..='5')) {
                    end += 1;
                }
                Some(SignKind::HorizontalEpisema)
            }
            'r' if matches!(chars.get(end), Some('1'..='8')) => {
                end += 1;
                Some(SignKind::Accent)
            }
            'r' => {
                if matches!(chars.get(end), Some('0')) {
                    end += 1;
                }
                Some(SignKind::Cavum)
            }
            'o' | 'O' => {
                if matches!(chars.get(end), Some('0' | '1' | '2')) {
                    end += 1;
                }
                Some(SignKind::Oriscus)
            }
            'w' | 'W' => Some(SignKind::Quilisma),
            'v' | 'V' => Some(SignKind::Virga),
            's' => {
                while matches!(chars.get(end), Some('s')) && end - index < 3 {
                    end += 1;
                }
                Some(SignKind::Stropha)
            }
            '~' | '<' | '>' => Some(SignKind::Liquescentia),
            '=' => Some(SignKind::Linea),
            'x' | 'X' | 'y' | 'Y' | '#' => {
                if matches!(chars.get(end), Some('?'))
                    || (char == '#' && matches!(chars.get(end), Some('#')))
                {
                    end += 1;
                }
                Some(SignKind::Alteration)
            }
            // Fusion `@`, spacing `/…`, `!` and shape hints `[…]` are kept
            // in the raw glyph but are not note signs (`Gabc.tex:603-700`).
            _ => None,
        };
        if let Some(kind) = kind {
            signs.push(NeumeSign::new(
                kind,
                chars[index..end].iter().collect::<String>(),
            ));
        }
        index = end;
    }
    signs
}

/// Splits a file at the `%%` line into its header and notation sections
/// (`Gabc.tex:51,58`; grammar `END_OF_DEFINITIONS` at `gabc-score-determination.y:741-745`).
///
/// The returned notation starts at the `%%` marker itself, so concatenating
/// the two halves reproduces the source bytes exactly (including comments,
/// which are significant to a faithful round trip).
pub fn split_sections(text: &str) -> Option<(String, String)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim() == "%%" {
            let marker = line.find("%%")?;
            let at = offset + marker;
            return Some((text[..at].to_owned(), text[at..].to_owned()));
        }
        offset += line.len();
    }
    None
}

/// Recognises a headerless GABC body: the notation half of a file with no
/// `%%` line, as exported raw by GregoBase and rendered by exsurge.
///
/// After leading whitespace and `%` comment lines, the text must begin either
/// with a clef-only group (`(c4)`, `(f3)`, `(cb3)`, `(c1@c4)`, …) or with a
/// syllable immediately followed by a parenthesised note group
/// (`Ky(f)ri(g)e`). Empty groups (`( )`) are skipped: the grammar allows
/// `notes: | notes note` (`gabc-score-determination.y:854-855`) and an empty
/// group before the first syllable is accepted by Gregorio when a header and
/// `%%` are prepended. A **clef** after such an empty group is not accepted:
/// Gregorio rejects it with "clef change is not supported on the first
/// syllable" (verified with its parser at commit `b63f074`). A header line
/// (`name:`, `mode:`, …) does not match, so a malformed file that has a header
/// but no separator stays an error.
///
/// Gregorio itself **rejects** such a file: `%%+(\n|\r)+` alone switches its
/// lexer from `INITIAL` to the `score` state
/// (`gabc-score-determination.l:225-228`) and the grammar requires
/// `END_OF_DEFINITIONS` before any syllable (`gabc-score-determination.y:737-749`).
/// This is a deliberate scoretk tolerance, recorded in `docs/agents/gabc.md` §15.
pub fn looks_like_gabc_body(text: &str) -> bool {
    let mut rest = text.trim_start();
    while let Some(next) = strip_body_comment(rest) {
        rest = next;
    }
    let mut skipped_empty = false;
    while let Some(after_group) = rest.strip_prefix('(') {
        let Some(end) = after_group.find(')') else {
            return false;
        };
        if !after_group[..end].trim().is_empty() {
            if skipped_empty && is_clef_group(&after_group[..end]) {
                return false;
            }
            return is_clef_group(&after_group[..end]);
        }
        skipped_empty = true;
        rest = after_group[end + 1..].trim_start();
    }
    let Some(open) = rest.find('(') else {
        return false;
    };
    let syllable = &rest[..open];
    if !syllable.chars().any(char::is_alphabetic)
        || syllable
            .chars()
            .any(|c| matches!(c, '\n' | '\r' | ')' | '<' | '{' | '\\' | '%'))
    {
        return false;
    }
    let code = &rest[open + 1..];
    let Some(end) = code.find(')') else {
        return false;
    };
    let code = code[..end].trim();
    is_clef_group(code) || code.chars().next().is_some_and(is_group_start)
}

/// Skips one whole-line `%` comment at the start of a body. A `%%` line is a
/// section separator, never a comment (`Gabc.tex:51,58`).
fn strip_body_comment(text: &str) -> Option<&str> {
    if text.starts_with('%') && !text.starts_with("%%") {
        let end = text.find('\n')?;
        Some(text[end + 1..].trim_start())
    } else {
        None
    }
}

/// True for a clef token or a pair of simultaneous clefs (`Gabc.tex:847-870`).
fn is_clef_group(code: &str) -> bool {
    let code = code.trim();
    !code.is_empty()
        && code.split('@').all(|part| {
            let bytes = part.as_bytes();
            matches!(bytes.first(), Some(b'c' | b'f'))
                && bytes.last().is_some_and(|byte| byte.is_ascii_digit())
                && (bytes.len() == 2 || (bytes.len() == 3 && bytes[1] == b'b'))
        })
}

/// First character of a note group: a pitch letter (`is_pitch`), a custos
/// (`Gabc.tex:872-906`) or a separation bar (`:801-845`).
fn is_group_start(c: char) -> bool {
    is_pitch(c) || matches!(c, ',' | ';' | ':' | '`' | '^' | 'z' | 'Z')
}

/// Parses the header lines (`Gabc.tex:67-74`).
///
/// A name is ASCII letters/digits with dashes; a value ends at `;` or `;;`,
/// and a multi-line value omits the first `;` before continuing.
pub fn parse_headers(header: &str) -> Vec<GregorianHeader> {
    parse_header_lines(header).0
}

/// Parses the header section into its fields **and** its exact layout.
///
/// A `%` comment and an empty line are header items in their own right
/// (`Gabc.tex:51,63-64`; lexer `INITIAL` → `inicomments` at
/// `gabc-score-determination.l:129-133`); `;;` is the terminator of a
/// multi-line value (`:69-74`) and is kept even when no continuation follows.
/// The returned field indices refer to the first element.
pub fn parse_header_layout(
    header: &str,
) -> (Vec<GregorianHeader>, Vec<GregorianHeaderItem>, Vec<usize>) {
    parse_header_lines(header)
}

fn parse_header_lines(
    header: &str,
) -> (Vec<GregorianHeader>, Vec<GregorianHeaderItem>, Vec<usize>) {
    let mut result: Vec<GregorianHeader> = Vec::new();
    let mut items: Vec<GregorianHeaderItem> = Vec::new();
    let mut doubles: Vec<usize> = Vec::new();
    let mut pending: Option<GregorianHeader> = None;
    let mut pending_double = false;
    for line in header.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            items.push(GregorianHeaderItem::Blank);
            continue;
        }
        if trimmed.starts_with('%') {
            items.push(GregorianHeaderItem::Comment(line.to_owned()));
            continue;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                continue;
            }
            let mut header = GregorianHeader {
                name: name.to_owned(),
                value: value.trim().to_owned(),
            };
            match finish_header(&mut header) {
                Some(is_double) => {
                    let index = result.len();
                    if is_double {
                        doubles.push(index);
                    }
                    result.push(header);
                    items.push(GregorianHeaderItem::Field(index));
                    pending = None;
                }
                None => {
                    let index = result.len();
                    items.push(GregorianHeaderItem::Field(index));
                    pending = Some(header);
                    pending_double = false;
                }
            }
        } else if let Some(header) = pending.as_mut() {
            header.value.push('\n');
            header.value.push_str(trimmed);
            if let Some(is_double) = finish_header(header) {
                pending_double |= is_double;
                if pending_double {
                    doubles.push(result.len());
                }
                result.push(pending.take().expect("pending header"));
            }
        }
    }
    (result, items, doubles)
}

/// Removes the terminating `;` / `;;` and reports whether it was a double one.
fn finish_header(header: &mut GregorianHeader) -> Option<bool> {
    if header.value.ends_with(";;") {
        header.value.truncate(header.value.len() - 2);
        Some(true)
    } else if header.value.ends_with(';') {
        header.value.pop();
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clefs_follow_gregorios_key_rule() {
        assert_eq!(staff_height('a'), 3);
        assert_eq!(staff_height('n'), 16);
        assert_eq!(staff_height('p'), 19); // `o` is skipped
        assert_eq!(staff_height('G'), staff_height('g'));
        assert_eq!(clef_key("c3"), 5);
        assert_eq!(clef_key("f3"), 2);
        assert_eq!(clef_key("cb3"), 5);
        assert_eq!(clef_key("c1@c4"), 1);
        assert_eq!(diatonic_step('g', "c3"), -1);
    }

    #[test]
    fn derived_midi_pitch_is_flagged_as_a_projection() {
        assert_eq!(concert_pitch(0), 60);
        assert_eq!(concert_pitch(1), 62);
        assert_eq!(tpc_of_step(0), 14);
    }

    #[test]
    fn only_pitch_letters_are_notes() {
        assert!(is_pitch('g'));
        assert!(is_pitch('P'));
        assert!(!is_pitch('o'));
        assert!(!is_pitch('z'));
    }

    #[test]
    fn clef_tokens_are_not_neume_glyphs() {
        assert_eq!(parse_clefs("c3) "), vec!["c3"]);
        assert_eq!(parse_clefs("gf3h"), Vec::<String>::new());
        assert_eq!(parse_clefs("c1@c4 g"), vec!["c1@c4"]);
        assert_eq!(parse_clefs("f3"), vec!["f3"]);
    }

    #[test]
    fn rhythmic_signs_keep_their_placement() {
        let signs = parse_signs(".'1_502..~r3");
        let raw: Vec<&str> = signs.iter().map(|sign| sign.raw.as_str()).collect();
        assert_eq!(raw, [".", "'1", "_502", "..", "~", "r3"]);
        assert_eq!(signs[1].kind, SignKind::Ictus);
        assert_eq!(signs[2].kind, SignKind::HorizontalEpisema);
        assert_eq!(signs[5].kind, SignKind::Accent);
    }

    #[test]
    fn headerless_bodies_are_recognised_without_matching_other_texts() {
        for body in [
            "(c4) Ky(f)ri(g)e(h) e(g)lé(f)i(e)son.(f) (::)\n",
            "(f3) VE(g.)ni(h') (,)\n",
            "(cb3) A(g) (::)\n",
            "(c1@c4) A(g) (::)\n",
            "\n  % a raw export\n  Ky(f)ri(g)e(h) (::)\n",
            // Empty groups are legal `notes` (gabc-score-determination.y:854-855).
            "( ) a(a) b(b) c(c) ( )\n",
            "() a(a) b(b) ( )\n",
        ] {
            assert!(looks_like_gabc_body(body), "{body:?}");
            assert!(split_sections(body).is_none(), "{body:?}");
        }
        for other in [
            "name: Veni;\n(c3) Ve(g)\n",
            "%abc-2.1\nX:1\nT:Test\nK:C\n",
            "\\version \"2.24.0\"\n{ c'4 }\n",
            "<?xml version=\"1.0\"?><score-partwise/>\n",
            "(hello) world\n",
            // Gregorio: "clef change is not supported on the first syllable".
            "() (c4) Vim(dh) (::)\n",
            "{ c'4 }\n",
        ] {
            assert!(!looks_like_gabc_body(other), "{other:?}");
        }
    }

    #[test]
    fn sections_and_headers_survive_comments_and_multiline_values() {
        let source =
            "% a header comment\nname: Factus;\nbook: Graduale\n  Romanum;;\n%%\n(c3) text(g)\n";
        let (header, notation) = split_sections(source).expect("two sections");
        assert_eq!(
            header,
            "% a header comment\nname: Factus;\nbook: Graduale\n  Romanum;;\n"
        );
        assert!(notation.starts_with("%%\n"));
        assert_eq!(format!("{header}{notation}"), source);
        let parsed = parse_headers(&header);
        assert_eq!(parsed[0].name, "name");
        assert_eq!(parsed[0].value, "Factus");
        assert_eq!(parsed[1].value, "Graduale\nRomanum");
    }
}
