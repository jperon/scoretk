// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : écriture d'un chant grégorien avec les porteurs
// signifiants de la norme ABC 2.1 (§3.1 titre, §4.6 clé, §4.14 décorations,
// §4.16 symboles redéfinissables, §5 paroles), réécrit en Rust en 2026.
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
//! spécification définit : `T:` (§3.1), `K:` avec `clef=` (§4.6),
//! `!symbole!` / `U:` (§4.14/§4.16) et `w:` (§5). Une annotation dont le
//! texte serait `text:`, `glyph:` ou `letter:` est un vidage de la source :
//! elle n'est pas un porteur et n'est pas écrite.

use std::fmt::Write as _;

use scoretk_abc_common::gregorian as carrier;
use scoretk_abc_common::text::{escape_annotation, escape_lyric};
use scoretk_core::model::{BarKind, Gregorian, GregorianNote, GroupTokenKind, Score, Syllable};
use scoretk_gabc_common::syntax::diatonic_step;

const STEPS: &[u8; 7] = b"cdefgab";

/// Rend un document ABC 2.1 complet depuis la charge grégorienne.
pub(crate) fn render(_score: &Score, gregorian: &Gregorian) -> String {
    let mut out = String::new();
    out.push_str("%abc-2.1\nX:1\n");
    if let Some(name) = gregorian
        .headers
        .iter()
        .find(|header| header.name == "name")
    {
        let _ = writeln!(out, "T:{}", escape_annotation(&name.value));
    } else {
        out.push_str("T:\n");
    }
    out.push_str("M:4/4\nL:1/8\n");
    // §4.6 : `clef=C3` est un paramètre ABC natif, pas une copie du jeton
    // source. Il permet au lecteur de reconstruire hauteur et position.
    let clef = gregorian.clefs.first().map_or("c4", String::as_str);
    let _ = writeln!(out, "{}", carrier::clef_field(clef));
    // §3.1 : `I:` porte une instruction. Le marqueur annonce le dialecte sans
    // préfixe privé ; il n'est pas un fait GABC.
    out.push_str(carrier::CHANT_MARKER);
    out.push('\n');
    for definition in carrier::definitions() {
        let _ = writeln!(out, "{definition}");
    }

    for syllable in &gregorian.syllables {
        let notes = write_syllable(&mut out, syllable, clef);
        if notes > 0 {
            // §5 : les paroles vont dans un champ `w:`, jamais dans une
            // annotation `"^…"` qui s'imprimerait au-dessus des notes. Un `_`
            // par note supplémentaire prolonge la syllabe (§2.1.3).
            let _ = writeln!(
                out,
                "w: {}{}",
                escape_lyric(&syllable.text),
                " _".repeat(notes - 1)
            );
        }
    }
    out
}

/// Écrit une ligne ABC par syllabe grégorienne et renvoie le nombre de notes.
fn write_syllable(out: &mut String, syllable: &Syllable, clef: &str) -> usize {
    if syllable.notes.is_empty() && syllable.bars.is_empty() && syllable.text.is_empty() {
        return 0;
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
        // Un attribut crocheté ou un signe de groupe n'a pas de porteur dans
        // la norme ABC : il est compté comme défaut, pas réduit à une étiquette.
        if token.text.starts_with('[') {
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
    // §4.8 : une barre de séparation est une barre ABC, pas une annotation
    // `^bar: ,`. Les variantes Dominicanes n'ont pas de porteur (§4.8).
    for bar in &syllable.bars {
        if let Some(symbol) = abc_bar(*bar) {
            let _ = write!(line, "{symbol} ");
        }
    }
    out.push_str(line.trim_end());
    out.push('\n');
    sung.len()
}

/// Barre GABC → barre ABC 2.1 §4.8 (porteur natif, pas une étiquette).
fn abc_bar(bar: BarKind) -> Option<&'static str> {
    match bar {
        BarKind::DivisioMinima
        | BarKind::DivisioMinimis
        | BarKind::DivisioMinimaLedger
        | BarKind::Virgula
        | BarKind::VirgulaLedger => Some("|"),
        BarKind::DivisioMinor => Some("||"),
        BarKind::DivisioMaior | BarKind::DivisioMaiorDot => Some("[|"),
        BarKind::DivisioFinalis => Some("::"),
        BarKind::Dominican(_) => None,
    }
}

/// Ajoute les décorations portant chaque caractère d'un glyphe GABC.
fn push_glyph(line: &mut String, glyph: &str) {
    for character in glyph.chars() {
        if character.is_whitespace() {
            continue;
        }
        if let Some(name) = carrier::glyph_name(character) {
            let _ = write!(line, "!{name}! ");
        }
        // Un caractère hors table n'a pas de décoration signifiante : il est
        // perdu, pas copié dans une annotation.
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
    fn gregorian_uses_standard_abc_carriers() {
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
        assert!(abc.contains("K:C clef=C3\n"), "{abc}");
        assert!(abc.contains("U: H = !fermata!\n"), "{abc}");
        assert!(abc.contains("!quilisma!"), "{abc}");
        assert!(abc.contains("w: San\\-ctus\n"), "{abc}");
        for dump in ["^text:", "^glyph:", "N:language=", "I:clef=", "gabc"] {
            assert!(!abc.contains(dump), "{dump} is a dump: {abc}");
        }
    }
}
