// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `src/gabc/gabc-score-determination.{y,l}` et de
// `doc/GregorioNabcRef.tex` de Gregorio (GPL-3.0-or-later), modifié (réécrit en
// Rust) en 2026.
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

//! Implicit NABC line count of a headerless body (raw GregoBase export).
//!
//! GregoBase stores bare notation bodies with no header, but renders them
//! after prepending `nabc-lines: 1;` to the file
//! (`specs/gabc/gregobase/chant_img.php:48`, function `gregorio()`). The
//! manuscript snippets separated by `|` are therefore present in the raw
//! export while the declaration that makes them legal is not. The GABC
//! reference spells out the alternating square/manuscript pattern and the
//! meaning of `nabc-lines: x;` (`doc/GregorioNabcRef.tex:153-170`).
//!
//! Gregorio's grammar reacts to a cut without the header as a **fatal error**:
//! both `NOTES NABC_CUT` and `NABC_CUT` call `gregorio_message(... VERBOSITY_FATAL)`
//! when `nabc_lines == 0`, with the message "You used character \"|\" in gabc
//! without setting \"nabc-lines\" parameter" (`gabc-score-determination.y:870-907`).
//! The cut cycles the state modulo `nabc_lines+1`
//! (`gabc-score-determination.y:879,906`), so the declared count must be at
//! least the greatest number of cuts in one group for the square snippets to
//! be read as notes again after the manuscript ones.
//!
//! The lexer emits `NABC_CUT` only in its `notes` state, between an opening
//! `(` and the closing `)` (`gabc-score-determination.l:385-396`). A `|` in
//! lyric text, in a header or after a `%` comment is not a cut
//! (`:240,313-320`). This detector follows the same states, including the
//! `\$.` escape (`:240`, `Gabc.tex:247-250`) and the verbatim `<v>…</v>` TeX
//! region (`:328-332`, `Gabc.tex:297-309`), so a pipe in those contexts cannot
//! invent a NABC voice.

/// Maximum number of unescaped `|` cuts in one parenthesised notation group,
/// or 0 when the body has none.
///
/// Called only when the source has no `nabc-lines` header: an explicit header
/// always wins, even `nabc-lines: 0;` (which Gregorio would reject on a cut,
/// but which scoretk keeps as the author's declaration).
pub(super) fn detect_nabc_lines(body: &str) -> usize {
    // All the tokens tested below (`(`, `)`, `|`, `%`, `$`, newlines and the
    // `<v>`/`</v>` delimiters) are ASCII. Scanning bytes is therefore UTF-8
    // safe: a non-ASCII byte can neither be a token nor start a token. Each
    // position does at most a bounded prefix comparison, so the whole body is
    // visited once, without copying its remainder.
    let bytes = body.as_bytes();
    let mut index = 0;
    let mut in_notes = false;
    let mut in_comment = false;
    let mut in_verbatim = false;
    let mut current = 0;
    let mut maximum = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_comment {
            if matches!(byte, b'\n' | b'\r') {
                in_comment = false;
            }
            index += 1;
            continue;
        }
        // `\$.` is one lexer token in both the `score` and `notes` states:
        // `$|` and `$(` are literal characters, not a cut or a group. A
        // multi-byte escaped character leaves only inert continuation bytes
        // if only its first byte is skipped.
        if byte == b'$' {
            index += 2;
            continue;
        }
        if in_verbatim {
            if byte == b'<' && bytes[index..].starts_with(b"</v>") {
                in_verbatim = false;
                index += 4;
            } else {
                index += 1;
            }
            continue;
        }
        if !in_notes && bytes[index..].starts_with(b"<v>") {
            in_verbatim = true;
            index += 3;
            continue;
        }
        if !in_notes && byte == b'%' {
            in_comment = true;
            index += 1;
            continue;
        }
        if byte == b'(' {
            in_notes = true;
            current = 0;
        } else if in_notes && byte == b')' {
            maximum = maximum.max(current);
            in_notes = false;
        } else if in_notes && byte == b'|' {
            current += 1;
        }
        index += 1;
    }
    maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maximum_of_one_group_wins() {
        assert_eq!(detect_nabc_lines("(e|ta)f(gh|pi|to)"), 2);
        assert_eq!(detect_nabc_lines("(e|ta) f(gh|pi)"), 1);
        assert_eq!(detect_nabc_lines("(efgh)"), 0);
        assert_eq!(detect_nabc_lines(""), 0);
    }

    #[test]
    fn pipes_outside_notation_are_not_cuts() {
        let source = "la|ti(f) % a | comment\nKy|ri(g) <v>a|b</v> $(c|d)\n";
        assert_eq!(detect_nabc_lines(source), 0);
        assert_eq!(detect_nabc_lines("(e$|ta)f(g|pi)"), 1);
    }

    #[test]
    fn a_cut_inside_a_group_after_a_comment_line_still_counts() {
        let source = "% | not a cut\n(e) % | still not a cut\n(f|ta)\n";
        assert_eq!(detect_nabc_lines(source), 1);
    }

    #[test]
    fn multibyte_text_and_escapes_keep_the_byte_scan_aligned() {
        let source = "岩(e|f) (g|h|i) é<v>x|y</v> $(j|k) $|(l|m)";
        assert_eq!(detect_nabc_lines(source), 2);
    }

    #[test]
    fn a_large_body_is_scanned_in_one_linear_pass() {
        // 25 000 groups of 8 bytes ≈ 200 KB. Rebuilding a `String` from the
        // remainder at every character made this take minutes on a real
        // GregoBase body, including in the wasm build used by Cantus.
        let body = "(ef|ta) ".repeat(25_000);
        let start = std::time::Instant::now();
        assert_eq!(detect_nabc_lines(&body), 1);
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "one pass over {} bytes took {elapsed:?}",
            body.len()
        );
    }
}
