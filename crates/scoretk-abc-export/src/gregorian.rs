// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : écriture d'un chant grégorien avec les porteurs
// signifiants de la norme ABC 2.1 (§3.1, §4.14, §4.16, §4.19), réécrit en
// Rust en 2026.
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

//! Sortie grégorienne additive du writer ABC.
//!
//! Le dialecte par défaut reste inchangé : ce module n'est appelé que lorsque
//! l'option `abc_gregorian` est posée *et* que la partition porte une charge
//! `Gregorian`. Chaque fait est porté par une construction ABC que sa
//! spécification définit : champ `N:`/`I:` (§3.1), décoration `!symbole!`
//! éventuellement définie par `U:` (§4.14/§4.16), annotation `"^…"` (§4.19).
//! Aucune donnée n'est codée en hexadécimal ni préfixée `gabc`.

use std::fmt::Write as _;

use scoretk_abc_common::gregorian as carrier;
use scoretk_abc_common::text::escape_annotation;
use scoretk_core::model::{Gregorian, GregorianNote, GroupTokenKind, Score, Syllable};
use scoretk_gabc_common::syntax::diatonic_step;

const STEPS: &[u8; 7] = b"cdefgab";

/// Rend un document ABC 2.1 complet depuis la charge grégorienne.
pub(crate) fn render(_score: &Score, gregorian: &Gregorian) -> String {
    let mut out = String::new();
    out.push_str("%abc-2.1\nX:1\n");
    if let Some(name) = gregorian.headers.iter().find(|header| header.name == "name") {
        let _ = writeln!(out, "T:{}", escape_annotation(&name.value));
    } else {
        out.push_str("T:\n");
    }
    out.push_str("M:4/4\nL:1/8\nK:C\n");
    // §3.1 : `I:` porte une instruction, `N:` une note libre. Le marqueur
    // `I:chant=gregorian` annonce le dialecte sans préfixe privé.
    out.push_str(carrier::CHANT_MARKER);
    out.push('\n');
    let clef = gregorian.clefs.first().map_or("c4", String::as_str);
    let _ = writeln!(out, "{}{}", carrier::CLEF_FIELD, clef);
    for definition in carrier::definitions() {
        let _ = writeln!(out, "{definition}");
    }
    // Un en-tête GABC est un fait atomique : un champ `N:` par fait, avec sa
    // valeur lisible (`Gabc.tex:67-154`, ABC 2.1 §3.1.11).
    for header in &gregorian.headers {
        let value = escape_annotation(&header.value.replace(['\n', '\r'], " "));
        let _ = writeln!(out, "N:{}={}", header.name, value);
    }

    for syllable in &gregorian.syllables {
        write_syllable(&mut out, syllable, clef);
    }
    out
}

/// Écrit une ligne ABC par syllabe gregorienne.
fn write_syllable(out: &mut String, syllable: &Syllable, clef: &str) {
    if syllable.notes.is_empty() && syllable.bars.is_empty() && syllable.text.is_empty() {
        return;
    }
    // §4.19 : la valeur lisible de la syllabe (y compris sa balise `<sp>`)
    // est une annotation above. Le lecteur sépare le texte et la balise.
    if !syllable.text.is_empty() {
        let _ = writeln!(
            out,
            "\"{}{}\"",
            carrier::TEXT_ANNOTATION,
            escape_annotation(&syllable.text)
        );
    }
    if let Some(above) = &syllable.above_text {
        let _ = writeln!(
            out,
            "\"{}{}\"",
            carrier::ABOVE_ANNOTATION,
            escape_annotation(above)
        );
    }
    for attribute in &syllable.leading_attributes {
        let _ = writeln!(
            out,
            "\"{}{}\"",
            carrier::ATTRIBUTE_ANNOTATION,
            escape_annotation(attribute)
        );
    }
    if let Some(change) = &syllable.clef {
        let _ = writeln!(
            out,
            "\"{}{}\"",
            carrier::CLEF_ANNOTATION,
            escape_annotation(change)
        );
    }

    let sung: Vec<&GregorianNote> = syllable
        .notes
        .iter()
        .filter(|note| note.letter != '\0')
        .collect();
    // Les jetons non-note sont replacés au nombre de notes chantées qui les
    // précédait : un signe d'espacement appartient au glyphe de la note
    // précédente, pas à la fin de la mesure.
    let mut suffixes: Vec<Vec<&str>> = vec![Vec::new(); sung.len() + 1];
    for token in &syllable.group_tokens {
        if matches!(
            token.kind,
            GroupTokenKind::Bar | GroupTokenKind::Clef | GroupTokenKind::Custos
        ) {
            continue;
        }
        suffixes[token.notes_before.min(sung.len())].push(&token.text);
    }
    let mut line = String::new();
    for (index, note) in sung.iter().enumerate() {
        // Décorations : une par caractère du glyphe source, avec son nom
        // signifiant (`!quilisma!`, `!spacing!`, …) ou son raccourci `U:`.
        push_glyph(&mut line, &note.glyph);
        for suffix in &suffixes[index] {
            push_glyph(&mut line, suffix);
        }
        for attribute in &note.attributes {
            let _ = write!(
                line,
                "\"{}{}\" ",
                carrier::ATTRIBUTE_ANNOTATION,
                escape_annotation(&attribute.text)
            );
        }
        for token in &syllable.group_tokens {
            if token.kind == GroupTokenKind::Custos && token.notes_before == index {
                let _ = write!(line, "!{}! ", carrier::custos_symbol(&token.text));
            }
        }
        let step = diatonic_step(note.letter, clef);
        let _ = write!(line, "{} ", pitch_name(step));
    }
    for suffix in &suffixes[sung.len()] {
        push_glyph(&mut line, suffix);
    }
    for token in &syllable.group_tokens {
        if token.kind == GroupTokenKind::Custos && token.notes_before >= sung.len() {
            let _ = write!(line, "!{}! ", carrier::custos_symbol(&token.text));
        }
    }
    for sign in &syllable.signs {
        let _ = write!(
            line,
            "\"{}{}\" ",
            carrier::SIGN_ANNOTATION,
            escape_annotation(&sign.raw)
        );
    }
    for bar in &syllable.bars {
        let _ = write!(
            line,
            "\"{}{}\" ",
            carrier::BAR_ANNOTATION,
            escape_annotation(bar.as_str())
        );
    }
    out.push_str(line.trim_end());
    out.push_str(" |\n");
}

/// Ajoute les décorations portant chaque caractère d'un glyphe GABC.
fn push_glyph(line: &mut String, glyph: &str) {
    for character in glyph.chars() {
        if character.is_whitespace() {
            continue;
        }
        // Un caractère hors table est écrit comme annotation lisible, jamais
        // comme un code : `^glyph: †` par exemple.
        if carrier::glyph_name(character).is_some() {
            let _ = write!(line, "{} ", carrier::glyph_token(character));
        } else {
            let _ = write!(line, "\"{}{character}\" ", carrier::GLYPH_ANNOTATION);
        }
    }
}

/// Inverse de `diatonic_step` : hauteur diatonique (C4 = 0) → note ABC.
fn pitch_name(step: i32) -> String {
    let name = STEPS[step.rem_euclid(7) as usize] as char;
    let octave = 4 + step.div_euclid(7);
    let mut result = name.to_string();
    if octave > 4 {
        result.push_str(&"'".repeat((octave - 4) as usize));
    } else if octave < 4 {
        result.push_str(&",".repeat((4 - octave) as usize));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{BarKind, GregorianHeader, GregorianNote, NeumeSign, SignKind};

    #[test]
    fn gregorian_uses_readable_abc_carriers() {
        let gregorian = Gregorian {
            headers: vec![GregorianHeader {
                name: "language".to_owned(),
                value: "latin".to_owned(),
            }],
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "San-ctus".to_owned(),
                notes: vec![GregorianNote {
                    letter: 'e',
                    glyph: "w".to_owned(),
                    signs: vec![NeumeSign::new(SignKind::Quilisma, "w")],
                    ..GregorianNote::default()
                }],
                bars: vec![BarKind::DivisioMinima],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let abc = render(&Score::default(), &gregorian);
        assert!(abc.contains("N:language=latin\n"));
        assert!(abc.contains("I:clef=c3\n"));
        assert!(abc.contains("U: H = !fermata!\n"));
        assert!(abc.contains("P "), "{abc}"); // quilisma = raccourci P
        assert!(abc.contains("\"^text: San-ctus\""));
        assert!(abc.contains("\"^bar: ,\""));
        assert!(!abc.contains("gabc"), "no private prefix: {abc}");
    }
}
