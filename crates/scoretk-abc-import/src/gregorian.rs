// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : lecture d'un document ABC portant un chant
// grégorien avec les porteurs signifiants de la norme ABC 2.1 (§3.1, §4.14,
// §4.16, §4.19), réécrit en Rust en 2026.
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
//! marqueur `I:chant=gregorian` écrit par le writer grégorien, et seulement
//! quand l'appelant demande explicitement ce mode. Les autres sources passent
//! par le chemin habituel, octet pour octet inchangé.

use scoretk_abc_common::gregorian as carrier;
use scoretk_core::model::{
    BarKind, Extension, Gregorian, GregorianHeader, GregorianNote, GroupToken, GroupTokenKind,
    NoteAttribute, Score, Syllable,
};
use scoretk_gabc_common::syntax::{clef_key, parse_signs};

/// Tente de lire un chant grégorien. `false` laisse la source au lecteur
/// ordinaire.
pub(crate) fn restore(src: &str, score: &mut Score) -> bool {
    if !src.lines().any(|line| line.trim() == carrier::CHANT_MARKER) {
        return false;
    }
    let mut clef = None;
    let mut headers = Vec::new();
    for line in src.lines() {
        if let Some(value) = line.strip_prefix(carrier::CLEF_FIELD) {
            clef = Some(value.trim().to_owned());
        } else if let Some(field) = line.strip_prefix("N:") {
            if let Some((name, value)) = field.split_once('=') {
                headers.push(GregorianHeader {
                    name: name.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
    }
    let Some(clef) = clef else {
        return false;
    };
    let mut voice = Voice {
        clef: clef.clone(),
        syllables: Vec::new(),
        glyph: String::new(),
        attributes: Vec::new(),
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
    rebuilt.report.losses.extend(score.report.losses.iter().cloned());
    rebuilt.extensions.format = Some(Extension::Gregorian(payload));
    *score = rebuilt;
    true
}

/// État de lecture d'une voix grégorienne.
struct Voice {
    /// Clé GABC annoncée par `I:clef=`.
    clef: String,
    /// Syllables reconstruites.
    syllables: Vec<Syllable>,
    /// Décorations de glyphe en attente de la prochaine note.
    glyph: String,
    /// Attributs crochetés en attente de la prochaine note.
    attributes: Vec<String>,
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
            self.syllables.push(Syllable {
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

    /// Traite un jeton ABC : annotation, décoration ou hauteur.
    fn token(&mut self, token: &str) {
        if let Some(annotation) = token.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
            if let Some(value) = annotation.strip_prefix(carrier::TEXT_ANNOTATION) {
                self.syllables.push(Syllable {
                    text: value.to_owned(),
                    space_after: true,
                    ..Syllable::default()
                });
            } else if let Some(value) = annotation.strip_prefix(carrier::ATTRIBUTE_ANNOTATION) {
                if self.syllables.is_empty() || self.glyph.is_empty() {
                    self.current().leading_attributes.push(value.to_owned());
                } else {
                    self.attributes.push(value.to_owned());
                }
            } else if let Some(value) = annotation.strip_prefix(carrier::ABOVE_ANNOTATION) {
                self.current().above_text = Some(value.to_owned());
            } else if let Some(value) = annotation.strip_prefix(carrier::CLEF_ANNOTATION) {
                let syllable = self.current();
                syllable.clef = Some(value.to_owned());
                syllable.group_tokens.push(GroupToken {
                    notes_before: syllable.notes.len(),
                    text: value.to_owned(),
                    kind: GroupTokenKind::Clef,
                });
            } else if let Some(value) = annotation.strip_prefix(carrier::BAR_ANNOTATION) {
                if let Some((bar, _)) = BarKind::parse_prefix(value) {
                    let syllable = self.current();
                    syllable.bars.push(bar);
                    syllable.group_tokens.push(GroupToken {
                        notes_before: syllable.notes.len(),
                        text: value.to_owned(),
                        kind: GroupTokenKind::Bar,
                    });
                }
            } else if let Some(value) = annotation.strip_prefix(carrier::SIGN_ANNOTATION) {
                // Un signe de groupe est porté par le glyphe de la dernière
                // note : le writer GABC le réémet après la barre de séparation.
                if let Some(last) = self.current().notes.last_mut() {
                    last.glyph.push_str(value);
                } else {
                    self.glyph.push_str(value);
                }
            } else if let Some(value) = annotation.strip_prefix(carrier::GLYPH_ANNOTATION) {
                self.glyph.push_str(value);
            }
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
        if is_pitch(token) {
            let glyph = std::mem::take(&mut self.glyph);
            let letter = letter_for_height(pitch_step(token) + clef_key(&self.clef) + 5);
            let attributes = std::mem::take(&mut self.attributes)
                .into_iter()
                .map(|text| NoteAttribute { after: 0, text })
                .collect();
            self.current().notes.push(GregorianNote {
                letter,
                signs: parse_signs(&glyph),
                glyph,
                attributes,
                ..GregorianNote::default()
            });
        }
    }
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
        && token[1..].chars().all(|character| character == '\'' || character == ',')
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
    fn readable_gregorian_abc_builds_a_gregorian_payload() {
        let src = "X:1\nT:k\nM:4/4\nL:1/8\nK:C\nI:chant=gregorian\nI:clef=c3\n\
                   N:language=latin\n\n\"^text: Sanctus\"\nP e |\n";
        let mut score = Score::default();
        assert!(restore(src, &mut score));
        let Some(Extension::Gregorian(payload)) = &score.extensions.format else {
            panic!("payload");
        };
        assert_eq!(payload.clefs, vec!["c3"]);
        assert_eq!(payload.headers[0].value, "latin");
        assert_eq!(payload.syllables[0].notes[0].glyph, "w");
        assert_eq!(payload.syllables[0].text, "Sanctus");
    }

    #[test]
    fn ordinary_abc_is_not_restored() {
        let mut score = Score::default();
        assert!(!restore("X:1\nT:k\nK:C\nC D|\n", &mut score));
    }
}
