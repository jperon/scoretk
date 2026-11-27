// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture d'un document ABC portant un chant
// grégorien avec les porteurs signifiants de la norme ABC 2.1 (§3.1 titre,
// §4.6 clé, §4.14 décorations, §4.16 symboles redéfinissables, §5 paroles),
// réécrit en Rust en 2026.
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

//! Lecteur additif du dialecte grégorien ABC.
//!
//! Il ne remplace pas l'analyseur ABC ordinaire : il ne s'active que sur le
//! marqueur `I:chant=gregorian` écrit par le writer grégorien. Les paroles
//! viennent des champs `w:` (§5), la clé du paramètre `clef=` de `K:` (§4.6)
//! et les signes des décorations `!…!` (§4.14). Les annotations étiquetées
//! (`^text:`, `^glyph:`, …) sont des vidages de la source : elles ne sont pas
//! lues. Les autres sources passent par le chemin habituel, inchangé.

use scoretk_abc_common::gregorian as carrier;
use scoretk_abc_common::text::unescape_lyric;
use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, GroupToken, GroupTokenKind,
    Score, Syllable,
};
use scoretk_gabc_common::syntax::{clef_key, parse_signs};

/// Tente de lire un chant grégorien. `false` laisse la source au lecteur
/// ordinaire.
pub(crate) fn restore(src: &str, score: &mut Score) -> bool {
    if !src.lines().any(|line| line.trim() == carrier::CHANT_MARKER) {
        return false;
    }
    let clef = src
        .lines()
        .find_map(carrier::clef_from_field)
        .unwrap_or_else(|| "c4".to_owned());
    // §3.1 : `T:` est le porteur natif du titre GABC (`name:`).
    let headers = src
        .lines()
        .find_map(|line| line.strip_prefix("T:"))
        .filter(|title| !title.trim().is_empty())
        .map(|title| {
            vec![GregorianHeader {
                name: "name".to_owned(),
                value: title.trim().to_owned(),
            }]
        })
        .unwrap_or_default();
    let mut voice = Voice {
        clef: clef.clone(),
        lyrics: src
            .lines()
            .filter_map(|line| line.strip_prefix("w:"))
            .map(lyric_text)
            .collect(),
        ..Voice::default()
    };
    voice.read(src);
    if voice.syllables.is_empty() || voice.note_count() == 0 {
        return false;
    }
    let payload = Gregorian {
        headers,
        staff_lines: 4,
        clefs: vec![clef],
        syllables: voice.syllables,
        derived_modern_view: true,
        ..Gregorian::default()
    };
    let mut rebuilt = payload.build_derived_score();
    rebuilt.metadata = score.metadata.clone();
    rebuilt
        .report
        .losses
        .extend(score.report.losses.iter().cloned());
    rebuilt.extensions.format = Some(Extension::Gregorian(payload));
    *score = rebuilt;
    true
}

/// État de lecture d'une voix grégorienne.
#[derive(Default)]
struct Voice {
    /// Clé GABC annoncée par le paramètre `clef=` de `K:`.
    clef: String,
    /// Paroles des champs `w:`, une entrée par syllabe chantée.
    lyrics: Vec<String>,
    /// Prochaine parole à consommer.
    lyric_index: usize,
    /// Syllables reconstruites.
    syllables: Vec<Syllable>,
    /// Décorations de glyphe en attente de la prochaine note.
    glyph: String,
}

impl Voice {
    /// Nombre de notes chantées déjà lues.
    fn note_count(&self) -> usize {
        self.syllables
            .iter()
            .map(|syllable| syllable.notes.len())
            .sum()
    }

    /// Syllable courante, créée au besoin.
    fn current(&mut self) -> &mut Syllable {
        if self.syllables.is_empty()
            || !self
                .syllables
                .last()
                .expect("syllable courante")
                .notes
                .is_empty()
        {
            let text = self
                .lyrics
                .get(self.lyric_index)
                .cloned()
                .unwrap_or_default();
            self.lyric_index += 1;
            self.syllables.push(Syllable {
                text,
                space_after: true,
                ..Syllable::default()
            });
        }
        self.syllables.last_mut().expect("syllable courante")
    }

    /// Lit le corps ABC, ligne par ligne.
    fn read(&mut self, src: &str) {
        for line in src.lines() {
            // Un champ d'en-tête ABC commence par une lettre puis `:`.
            let bytes = line.as_bytes();
            if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                continue;
            }
            if line.trim_start().starts_with("U:") || line.trim().starts_with('%') {
                continue;
            }
            for token in tokens(line) {
                self.token(token);
            }
        }
    }

    /// Traite un jeton ABC : décoration ou hauteur.
    fn token(&mut self, token: &str) {
        // Une annotation `"^…"` dont le contenu est une étiquette est un
        // vidage de la source : elle n'est jamais un porteur.
        if token.starts_with('"') {
            return;
        }
        if token.starts_with('!') && token.ends_with('!') {
            if let Some(raw) = custos_raw(token) {
                let syllable = self.current();
                syllable.group_tokens.push(GroupToken {
                    notes_before: syllable.notes.len(),
                    text: raw.to_owned(),
                    kind: GroupTokenKind::Custos,
                });
            } else if let Some(glyph) = carrier::glyph_char(token) {
                self.glyph.push(glyph);
            }
            return;
        }
        // Raccourcis `U:` (`H`, `L`, `M`, `P`) écrits comme jetons nus.
        if let Some(glyph) = carrier::glyph_char(token) {
            self.glyph.push(glyph);
            return;
        }
        // §4.8 : les barres ABC natives sont relues telles quelles ; une
        // annotation `^bar:` ne l'est pas (vidage de la source).
        let bar = match token {
            "|" => Some(BarKind::DivisioMinima),
            "||" => Some(BarKind::DivisioMinor),
            "[|" => Some(BarKind::DivisioMaior),
            "::" => Some(BarKind::DivisioFinalis),
            _ => None,
        };
        if let Some(bar) = bar {
            self.current().bars.push(bar);
            return;
        }
        if is_pitch(token) {
            let glyph = std::mem::take(&mut self.glyph);
            let letter = letter_for_height(pitch_step(token) + clef_key(&self.clef) + 5);
            self.current().notes.push(GregorianNote {
                letter,
                signs: parse_signs(&glyph),
                glyph,
                ..GregorianNote::default()
            });
        }
    }
}

/// Texte d'un champ `w:`, sans les marqueurs de mélisme ajoutés par le writer.
fn lyric_text(line: &str) -> String {
    let text = unescape_lyric(line.trim_start());
    text.trim()
        .trim_end_matches(|character: char| character == '_' || character.is_whitespace())
        .trim_end_matches('-')
        .trim()
        .to_owned()
}

/// Orthographe GABC d'une décoration de custos.
fn custos_raw(token: &str) -> Option<&'static str> {
    let name = token.strip_prefix('!')?.strip_suffix('!')?;
    match name {
        "custos" => Some("z"),
        "custos-up" => Some("Z"),
        "custos-zero" => Some("z0"),
        "custos-up-zero" => Some("Z0"),
        _ => None,
    }
}

/// Découpe une ligne en jetons ABC : une décoration `!…!` ou une annotation
/// `"…"` peut être immédiatement suivie de la note qu'elle affecte, sans
/// espace (ABC 2.1 §4.20).
fn tokens(line: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let bytes = rest.as_bytes();
        let start = match bytes[0] {
            b'!' | b'"' => 0,
            byte if byte.is_ascii_whitespace() => {
                rest = rest.trim_start();
                continue;
            }
            _ => rest
                .bytes()
                .position(|byte| byte.is_ascii_whitespace() || byte == b'!' || byte == b'"')
                .unwrap_or(rest.len()),
        };
        if start > 0 {
            result.push(&rest[..start]);
            rest = &rest[start..];
            continue;
        }
        let delimiter = bytes[0];
        let end = rest[1..]
            .find(delimiter as char)
            .map(|offset| offset + 2)
            .unwrap_or(rest.len());
        result.push(&rest[..end]);
        rest = &rest[end..];
    }
    result
}

/// Vrai pour une hauteur ABC (`c`…`b` avec octave).
fn is_pitch(token: &str) -> bool {
    token.starts_with(['c', 'd', 'e', 'f', 'g', 'a', 'b'])
        && token[1..]
            .chars()
            .all(|character| character == '\'' || character == ',')
}

/// Hauteur diatonique d'une note ABC (C4 = 0).
fn pitch_step(token: &str) -> i32 {
    const STEPS: &str = "cdefgab";
    let index = STEPS.find(token.chars().next().unwrap_or('c')).unwrap_or(0) as i32;
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

/// Inverse de la hauteur GABC : hauteur diatonique → lettre de position.
fn letter_for_height(height: i32) -> char {
    if !(3..=19).contains(&height) {
        return 'g';
    }
    let mut offset = height - 3;
    if offset >= 15 {
        offset += 1; // `o` n'est pas une lettre GABC (`src/dump/dump.c:78-88`).
    }
    (b'a' + offset as u8) as char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_gregorian_abc_builds_a_gregorian_payload() {
        let src = "X:1\nT:k\nM:4/4\nL:1/8\nK:C clef=C3\nI:chant=gregorian\n\
                   U: P = !uppermordent!\n\nP e |\nw: Sanctus\n";
        let mut score = Score::default();
        assert!(restore(src, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.clefs, vec!["c3"]);
        assert_eq!(payload.headers[0].value, "k");
        assert_eq!(payload.syllables[0].notes[0].glyph, "w");
        assert_eq!(payload.syllables[0].text, "Sanctus");
    }

    #[test]
    fn dump_annotations_are_not_read() {
        let src = "X:1\nT:k\nK:C\nI:chant=gregorian\n\"^text: Sanctus\"\n\
                   \"^glyph: virga\"\ne |\n";
        let mut score = Score::default();
        assert!(restore(src, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.syllables[0].text, "");
        assert_eq!(payload.syllables[0].notes[0].glyph, "");
    }

    #[test]
    fn ordinary_abc_is_not_restored() {
        let mut score = Score::default();
        assert!(!restore("X:1\nT:k\nK:C\nC D|\n", &mut score));
    }
}
