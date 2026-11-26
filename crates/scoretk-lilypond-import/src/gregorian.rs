// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture du dialecte grégorien du *Notation
// Reference* §17.4 (`VaticanaVoice`, clefs vaticanes, modificateurs de têtes
// de note, articulations, divisiones, ligatures `\[ … \]`, paroles
// `\lyricsto`), réécrit en Rust en 2026.
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

//! Lecteur additif d'une source LilyPond en ancienne notation grégorienne.
//!
//! Il ne remplace pas l'analyseur général : il ne s'active que sur un
//! `\new VaticanaVoice` et reconstruit le payload L6 à partir des porteurs
//! standard de LilyPond, sans aucun champ privé. Les autres sources passent
//! par le chemin habituel, octet pour octet inchangé.

use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, Score, Syllable,
};
use scoretk_gabc_common::syntax::{clef_key, parse_signs};

/// Tente de lire un document grégorien. `false` laisse la source au lecteur
/// ordinaire.
pub(crate) fn restore(src: &str, score: &mut Score) -> bool {
    let Some(voice) = braced(src, "\\new VaticanaVoice") else {
        return false;
    };
    let headers = header_fields(src);
    let lyrics = lyric_tokens(src);
    let mut reader = Voice::default();
    reader.read(voice);
    if reader.notes.is_empty() {
        return false;
    }
    let clef = reader.clef.clone().unwrap_or_else(|| "c4".to_owned());
    let mut syllables: Vec<Syllable> = Vec::new();
    for (index, note) in reader.notes.into_iter().enumerate() {
        let lyric = lyrics.get(index);
        let text = lyric.and_then(|token| token.text.clone());
        if lyric.is_some_and(|token| token.continuation) {
            if let Some(previous) = syllables.last_mut() {
                previous.space_after = false;
            }
        }
        if syllables.is_empty() || text.is_some() {
            syllables.push(Syllable {
                space_after: true,
                ..Syllable::default()
            });
        }
        if let Some(text) = text {
            syllables.last_mut().expect("syllable").text = text;
        }
        syllables.last_mut().expect("syllable").notes.push(note);
        for bar in reader
            .bars
            .iter()
            .filter(|(at, _)| *at == index + 1)
            .map(|(_, bar)| *bar)
        {
            syllables.last_mut().expect("syllable").bars.push(bar);
        }
    }
    for (at, bar) in reader.bars {
        if at > reader_note_count(&syllables) {
            if syllables.is_empty() {
                syllables.push(Syllable::default());
            }
            syllables.last_mut().expect("syllable").bars.push(bar);
        }
    }
    let payload = Gregorian {
        headers,
        staff_lines: 4,
        clefs: vec![clef],
        syllables,
        opening_separator: None,
        derived_modern_view: true,
        ..Gregorian::default()
    };
    let mut rebuilt = payload.build_derived_score();
    rebuilt.metadata = score.metadata.clone();
    for loss in &score.report.losses {
        rebuilt.report.losses.push(loss.clone());
    }
    rebuilt.extensions.format = Some(Extension::Gregorian(payload));
    *score = rebuilt;
    true
}

fn reader_note_count(syllables: &[Syllable]) -> usize {
    syllables.iter().map(|syllable| syllable.notes.len()).sum()
}

/// Contenu d'un bloc `{ … }` suivant un marqueur, sans imbrication : la
/// musique grégorienne écrite par `writer::gregorian` n'en a pas.
fn braced<'a>(src: &'a str, marker: &str) -> Option<&'a str> {
    let start = src.find(marker)?;
    let open = src[start..].find('{')? + start;
    let mut depth = 0usize;
    for (offset, character) in src[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open + 1..open + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

fn header_fields(src: &str) -> Vec<GregorianHeader> {
    let Some(header) = braced(src, "\\header") else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for line in header.lines() {
        let Some((name, value)) = line.trim().split_once('=') else {
            continue;
        };
        if !name.trim().starts_with("gabcHeader") {
            continue;
        }
        let value = unescape(value.trim().trim_matches('"'));
        if let Some((key, content)) = value.split_once('=') {
            result.push(GregorianHeader {
                name: key.to_owned(),
                value: content.to_owned(),
            });
        }
    }
    result
}

#[derive(Default)]
struct LyricToken {
    text: Option<String>,
    continuation: bool,
}

/// Découpe les paroles en jetons alignés sur les notes (`\lyricsto`).
fn lyric_tokens(src: &str) -> Vec<LyricToken> {
    let Some(body) = braced(src, "\\new VaticanaLyrics") else {
        return Vec::new();
    };
    let mut tokens = Vec::new();
    let mut chars = body.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' => {
                let mut text = String::new();
                while let Some(next) = chars.next() {
                    if next == '"' {
                        break;
                    }
                    if next == '\\' {
                        if let Some(escaped) = chars.next() {
                            text.push(escaped);
                        }
                    } else {
                        text.push(next);
                    }
                }
                tokens.push(LyricToken {
                    text: Some(text),
                    continuation: false,
                });
            }
            c if c.is_whitespace() => {}
            _ => {
                // `--` (continuation du mot) ou `_` (mélisme).
                let mut token = character.to_string();
                while chars.peek().is_some_and(|next| !next.is_whitespace()) {
                    token.push(chars.next().expect("character"));
                }
                tokens.push(LyricToken {
                    text: None,
                    continuation: token.starts_with("--"),
                });
            }
        }
    }
    tokens
}

#[derive(Default)]
struct Voice {
    clef: Option<String>,
    notes: Vec<GregorianNote>,
    bars: Vec<(usize, BarKind)>,
}

impl Voice {
    fn read(&mut self, body: &str) {
        let mut glyph = String::new();
        let mut inclinatum = false;
        let mut tokens = body.split_whitespace().peekable();
        while let Some(token) = tokens.next() {
            match token {
                "\\clef" => {
                    if let Some(name) = tokens.next() {
                        self.clef = Some(gabc_clef(name.trim_matches('"')));
                    }
                }
                "\\[" | "\\]" | "\\pes" | "\\flexa" | "\\melisma" | "\\melismaEnd" => {}
                "\\inclinatum" => inclinatum = true,
                "\\quilisma" => glyph.push('w'),
                "\\virga" => glyph.push('v'),
                "\\stropha" => glyph.push('s'),
                "\\oriscus" => glyph.push('o'),
                "\\deminutum" => glyph.push('~'),
                "\\auctum" => glyph.push('<'),
                "\\descendens" => glyph.push('>'),
                "\\linea" => glyph.push('='),
                "\\cavum" => glyph.push('r'),
                "\\augmentum" => glyph.push('.'),
                "\\ictus" => glyph.push('\''),
                "\\episemInitium" | "\\episemFinis" => {
                    if !glyph.contains('_') {
                        glyph.push('_');
                    }
                }
                "\\divisioMinima" => self.push_bar(BarKind::DivisioMinima),
                "\\divisioMaior" => self.push_bar(BarKind::DivisioMinor),
                "\\divisioMaxima" => self.push_bar(BarKind::DivisioMaior),
                "\\caesura" => self.push_bar(BarKind::Virgula),
                "\\finalis" => self.push_bar(BarKind::DivisioFinalis),
                token if is_pitch(token) => {
                    let clef = self.clef.clone().unwrap_or_else(|| "c4".to_owned());
                    let step = pitch_step(token);
                    let height = step + clef_key(&clef) + 5;
                    let mut letter = letter_for_height(height).unwrap_or('g');
                    if inclinatum {
                        letter = letter.to_ascii_uppercase();
                    }
                    self.notes.push(GregorianNote {
                        letter,
                        signs: parse_signs(&glyph),
                        glyph: std::mem::take(&mut glyph),
                        ..GregorianNote::default()
                    });
                    inclinatum = false;
                }
                _ => {}
            }
        }
    }

    fn push_bar(&mut self, bar: BarKind) {
        self.bars.push((self.notes.len(), bar));
    }
}

fn is_pitch(token: &str) -> bool {
    token.starts_with(['c', 'd', 'e', 'f', 'g', 'a', 'b'])
        && token[1..].chars().all(|c| c == '\'' || c == ',')
}

fn pitch_step(token: &str) -> i32 {
    const STEPS: &str = "cdefgab";
    let name = token.chars().next().unwrap_or('c');
    let index = STEPS.find(name).unwrap_or(0) as i32;
    let mut octave = 4;
    for mark in token.chars().skip(1) {
        match mark {
            '\'' => octave += 1,
            ',' => octave -= 1,
            _ => {}
        }
    }
    (octave - 4) * 7 + index
}

fn letter_for_height(height: i32) -> Option<char> {
    if !(3..=19).contains(&height) {
        return None;
    }
    let mut offset = height - 3;
    if offset >= 15 {
        offset += 1; // `o` n'est pas une lettre GABC.
    }
    Some((b'a' + offset as u8) as char)
}

fn gabc_clef(name: &str) -> String {
    let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
    let line = if digits.is_empty() { "4" } else { &digits };
    if name.starts_with("vaticana-do") {
        format!("c{line}")
    } else if name.starts_with("vaticana-fa") {
        format!("f{line}")
    } else {
        name.to_owned()
    }
}

fn unescape(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some(other) => result.push(other),
                None => result.push('\\'),
            }
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vaticana_clef_names_and_pitch_marks_reconstruct_gabc_letters() {
        assert_eq!(gabc_clef("vaticana-do3"), "c3");
        assert_eq!(gabc_clef("vaticana-fa2"), "f2");
        assert_eq!(pitch_step("c"), 0);
        assert_eq!(pitch_step("c'"), 7);
        assert_eq!(pitch_step("c,"), -7);
        assert_eq!(letter_for_height(7), Some('e'));
    }

    #[test]
    fn a_vaticana_source_builds_a_gregorian_payload() {
        let src = r#"\version "2.24.0"
\header { gabcHeader0 = "name=Kyrie" }
\score {
  <<
    \new VaticanaVoice = "cantus" {
      \clef "vaticana-do3"
      \[ \quilisma e' \flexa g' \] \divisioMinima
    }
    \new VaticanaLyrics \lyricsto "cantus" { "Ky" _ }
  >>
}
"#;
        let mut score = Score::default();
        assert!(restore(src, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.clefs, vec!["c3"]);
        assert_eq!(payload.headers[0].value, "Kyrie");
        assert_eq!(payload.syllables[0].notes.len(), 2);
        assert_eq!(payload.syllables[0].notes[0].glyph, "w");
        assert_eq!(payload.syllables[0].bars, vec![BarKind::DivisioMinima]);
    }
}
