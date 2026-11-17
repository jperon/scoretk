// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// La syntaxe des paroles `w:` suit la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>) ; aucune ligne de
// `abc2xml.py` de Willem Vree n'est copiée.
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

//! Paroles ABC (`w:`).
//!
//! Une ligne `w:` s'aligne sur les notes de la ligne de musique précédente.
//! Les syllabes sont séparées par des espaces ; `*` saute une note, `_`
//! prolonge la syllabe précédente, un tiret final marque la césure. Un tiret
//! littéral s'écrit `\-`.

use mscz2xml::model::{ChordRest, Fraction, Lyrics, LyricsSyllabic};

use crate::error::Warning;
use crate::reader::Voice;
use crate::text;

/// Applique une ligne `w:` à la dernière ligne de musique.
pub(crate) fn apply(voice: &mut Voice, value: &str, line: usize, warnings: &mut Vec<Warning>) {
    let tokens = split_tokens(value);
    if tokens.is_empty() {
        return;
    }
    let verse = voice.next_verse;
    voice.next_verse += 1;
    let chords = voice.last_line_chords.clone();
    let mut token_index = 0usize;
    let mut pending_word = false;

    for (measure, event) in chords {
        let Some(chord_rest) = event_mut(voice, measure, event) else {
            continue;
        };
        if !chord_rest.is_chord() {
            continue;
        }
        let Some(token) = tokens.get(token_index) else {
            break;
        };
        token_index += 1;
        if token == "*" {
            continue;
        }
        if token == "_" {
            if let Some(lyrics) = chord_rest.lyrics.last_mut() {
                lyrics.melisma = Some(Fraction::QUARTER);
            }
            continue;
        }
        let (mut syllable, hyphen, melisma) = dissociation(token);
        if syllable.is_empty() {
            continue;
        }
        if melisma {
            syllable.push('_');
        }
        let syllabic = if hyphen {
            if pending_word {
                LyricsSyllabic::Middle
            } else {
                LyricsSyllabic::Begin
            }
        } else if pending_word {
            LyricsSyllabic::End
        } else {
            LyricsSyllabic::Single
        };
        pending_word = hyphen;
        if melisma {
            syllable.pop();
        }
        let mut lyrics = Lyrics::new(verse, syllabic, text::unescape_lyric(&syllable));
        if melisma {
            lyrics.melisma = Some(Fraction::QUARTER);
        }
        chord_rest.lyrics.push(lyrics);
    }

    if token_index < tokens.len() {
        warnings.push(Warning::new(
            line,
            "plus de syllabes que de notes sur la ligne de musique",
        ));
    }
}

/// Découpe une ligne `w:` en syllabes, en respectant `\ ` et `\\`.
fn split_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' => {
                current.push(character);
                escaped = true;
            }
            character if character.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Sépare une syllabe de sa césure et de sa prolongation.
fn dissociation(token: &str) -> (String, bool, bool) {
    let mut value = token.to_owned();
    let mut melisma = false;
    let mut hyphen = false;
    if value.ends_with('_') && !value.ends_with("\\_") {
        value.pop();
        melisma = true;
    }
    if value.ends_with('-') && !value.ends_with("\\-") {
        value.pop();
        hyphen = true;
    }
    (value, hyphen, melisma)
}

/// Accès mutable à un accord par `(mesure, événement)`.
fn event_mut(voice: &mut Voice, measure: usize, index: usize) -> Option<&mut ChordRest> {
    if measure < voice.measures.len() {
        voice
            .measures
            .get_mut(measure)
            .and_then(|measure| measure.events.get_mut(index))
            .map(|event| &mut event.chord_rest)
    } else {
        voice
            .current
            .events
            .get_mut(index)
            .map(|event| &mut event.chord_rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupage() {
        assert_eq!(split_tokens("Bon\\ jour toi"), ["Bon\\ jour", "toi"]);
        assert_eq!(split_tokens("a b"), ["a", "b"]);
        assert!(split_tokens("   ").is_empty());
    }

    #[test]
    fn cesures() {
        assert_eq!(dissociation("Bon-"), ("Bon".into(), true, false));
        assert_eq!(dissociation("jour"), ("jour".into(), false, false));
        assert_eq!(dissociation("A_"), ("A".into(), false, true));
        assert_eq!(dissociation("hi\\-mid"), ("hi\\-mid".into(), false, false));
    }
}
