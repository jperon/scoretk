// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : écriture d'un chant grégorien dans le dialecte
// d'ancienne notation de LilyPond décrit par le *Notation Reference* §17.4
// (`VaticanaScore`, `VaticanaVoice`, `VaticanaLyrics`, `§17.4.4 Divisiones`,
// `§17.4.5 Gregorian articulation signs`, `§17.4.7 Gregorian square neume
// ligatures`, `§17.2.3 Custodes`), réécrit en Rust en 2026.
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

//! Sortie grégorienne additive du writer LilyPond.
//!
//! Le dialecte par défaut reste `\fixed` et ses octets ne changent pas. Ce
//! module n'est appelé que lorsque l'option `ly_gregorian` est posée *et* que
//! la partition porte une charge `Gregorian`. Les porteurs utilisés sont ceux
//! de la spécification LilyPond, pas des champs privés : clefs vaticanes,
//! modificateurs de têtes de note, articulations grégoriennes, divisiones,
//! ligatures `\[ … \]` et paroles `\lyricsto`.

use std::fmt::Write as _;

use scoretk_core::model::{
    BarKind, Gregorian, GregorianNote, GroupTokenKind, Score, SignKind, Syllable,
};
use scoretk_gabc_common::syntax::diatonic_step;
use scoretk_lilypond_common::gregorian as carrier;

const STEPS: &[u8; 7] = b"cdefgab";

/// Rend un document LilyPond complet en notation grégorienne.
pub(crate) fn render(score: &Score, gregorian: &Gregorian, version: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "\\version \"{version}\"");
    out.push_str("\\language \"english\"\n\n");
    write_header(&mut out, gregorian);
    out.push_str("\\score {\n  <<\n");
    out.push_str("    \\new VaticanaVoice = \"cantus\" {\n");
    let clef = gregorian.clefs.first().map(String::as_str).unwrap_or("c4");
    let _ = writeln!(out, "      \\clef \"{}\"", lily_clef(clef));
    for syllable in &gregorian.syllables {
        write_syllable(&mut out, syllable, clef);
    }
    out.push_str("    }\n");
    write_lyrics(&mut out, &gregorian.syllables);
    out.push_str("  >>\n");
    // §17.2.3 : un custos est produit par le `Custos_engraver` du Staff.
    out.push_str("  \\layout {\n    \\context {\n      \\Staff\n      \\consists Custos_engraver\n    }\n  }\n");
    out.push_str("}\n");
    let _ = score; // les métadonnées du modèle sont déjà dans les en-têtes L6
    out
}

/// `\header` accepte des champs définis par l'utilisateur (§21.2.3). Le nom
/// d'origine est porté dans la valeur pour les en-têtes dont l'identifiant
/// LilyPond n'est pas disponible (`office-part`, `user-notes`, …).
fn write_header(out: &mut String, gregorian: &Gregorian) {
    out.push_str("\\header {\n");
    if let Some(name) = gregorian.headers.iter().find(|h| h.name == "name") {
        let _ = writeln!(out, "  title = \"{}\"", escape(&name.value));
    }
    for (index, header) in gregorian.headers.iter().enumerate() {
        // Un identifiant LilyPond ne peut pas finir par un chiffre : on utilise
        // des lettres (`chantHeaderA`, `chantHeaderB`…), jamais un préfixe
        // `gabc` réservé au format source.
        let _ = writeln!(
            out,
            "  {} = \"{}\"",
            header_name(index),
            escape(&format!("{}={}", header.name, header.value))
        );
    }
    out.push_str("}\n\n");
}

fn header_name(index: usize) -> String {
    let mut name = String::from("chantHeader");
    let mut value = index;
    loop {
        name.push((b'A' + (value % 26) as u8) as char);
        value /= 26;
        if value == 0 {
            return name;
        }
        value -= 1;
    }
}

fn write_syllable(out: &mut String, syllable: &Syllable, clef: &str) {
    let sung: Vec<&GregorianNote> = syllable
        .notes
        .iter()
        .filter(|note| note.letter != '\0')
        .collect();
    if sung.is_empty() && syllable.bars.is_empty() {
        return;
    }
    // §17.4.7 : même un neume à une seule tête (virga, cavum…) se place
    // entre `\[` et `\]`; seul le punctum nu s'en passe.
    let ligature = sung.len() > 1
        || sung
            .iter()
            .any(|note| !note.glyph.is_empty() || note.letter.is_ascii_uppercase());
    if ligature {
        out.push_str("      \\[ ");
    }
    // §"Text annotations" : l'orthographe exacte des glyphes que le style
    // Vatican ne nomme pas (espacements, fusions), les attributs crochetés,
    // les custos et les textes au-dessus de la portée sont écrits comme des
    // annotations de note **lisibles**, jamais comme des codes.
    let mut suffixes: Vec<Vec<&str>> = vec![Vec::new(); sung.len() + 1];
    let mut custos: Vec<Vec<&str>> = vec![Vec::new(); sung.len() + 1];
    for token in &syllable.group_tokens {
        let at = token.notes_before.min(sung.len());
        if token.kind == GroupTokenKind::Custos {
            custos[at].push(&token.text);
        } else if !matches!(token.kind, GroupTokenKind::Bar | GroupTokenKind::Clef) {
            suffixes[at].push(&token.text);
        }
    }
    let mut previous_step = None;
    for (index, note) in sung.iter().enumerate() {
        if index > 0 {
            let step = diatonic_step(note.letter, clef);
            let join = match previous_step {
                Some(before) if step > before => " \\pes ",
                Some(_) => " \\flexa ",
                None => " ",
            };
            out.push_str(join);
        }
        previous_step = Some(diatonic_step(note.letter, clef));
        write_prefixes(out, note);
        let _ = write!(out, "{}", pitch_name(diatonic_step(note.letter, clef)));
        write_postfixes(out, note);
        let mut glyph = note.glyph.clone();
        for suffix in &suffixes[index] {
            glyph.push_str(suffix);
        }
        let mut facts: Vec<(&str, String)> = Vec::new();
        facts.push(("glyph", carrier::glyph_label(&glyph)));
        for attribute in &note.attributes {
            facts.push(("attribute", attribute.text.clone()));
        }
        for raw in &custos[index] {
            facts.push(("custos", (*raw).to_owned()));
        }
        if index + 1 == sung.len() {
            for attribute in &syllable.leading_attributes {
                facts.push(("attribute", attribute.clone()));
            }
            for suffix in &suffixes[sung.len()] {
                facts.push(("sign", (*suffix).to_owned()));
            }
            for sign in &syllable.signs {
                facts.push(("sign", sign.raw.clone()));
            }
            if let Some(change) = &syllable.clef {
                facts.push(("clef", change.clone()));
            }
            if let Some(above) = &syllable.above_text {
                facts.push(("above", above.clone()));
            }
            for raw in &custos[sung.len()] {
                facts.push(("custos", (*raw).to_owned()));
            }
        }
        if !facts.is_empty() {
            let annotation = carrier::annotation(&facts);
            let _ = write!(out, "{annotation}");
        }
    }
    if ligature {
        out.push_str(" \\]\n");
    } else if !sung.is_empty() {
        out.push('\n');
    }
    for bar in &syllable.bars {
        let _ = writeln!(out, "      {}", lily_bar(*bar));
    }
}

/// Préfixes de tête de note (§17.4.7) et signes grégoriens (§17.4.5/§17.4.6).
/// L'ordre suit celui des caractères du glyphe source : le lecteuradditif
/// reconstruit le jeton GABC dans le même ordre.
fn write_prefixes(out: &mut String, note: &GregorianNote) {
    if note.letter.is_ascii_uppercase() {
        out.push_str("\\inclinatum ");
    }
    for sign in &note.signs {
        let command = match (sign.kind, sign.raw.chars().next()) {
            (SignKind::Quilisma, _) => Some("\\quilisma"),
            (SignKind::Virga, Some('v' | 'V')) => Some("\\virga"),
            (SignKind::Stropha, Some('s' | 'S')) => Some("\\stropha"),
            (SignKind::Oriscus, Some('o' | 'O')) => Some("\\oriscus"),
            (SignKind::Liquescentia, Some('~')) => Some("\\deminutum"),
            (SignKind::Liquescentia, Some('<')) => Some("\\auctum"),
            (SignKind::Liquescentia, Some('>')) => Some("\\descendens"),
            (SignKind::Linea, _) => Some("\\linea"),
            (SignKind::Cavum, _) => Some("\\cavum"),
            (SignKind::PunctumMora, _) => Some("\\augmentum"),
            (SignKind::Ictus, _) => Some("\\ictus"),
            _ => None,
        };
        if let Some(command) = command {
            let _ = write!(out, "{command} ");
        }
    }
}

fn write_postfixes(out: &mut String, note: &GregorianNote) {
    for sign in &note.signs {
        if sign.kind == SignKind::HorizontalEpisema {
            out.push_str(" \\episemInitium \\episemFinis");
        }
    }
}

/// `\lyricsto` aligne un jeton par note : une chaîne `"…"` (même vide) ouvre
/// une syllabe, les suivantes reçoivent `_` (mélisme, §2.1.3). Le tiret `--`
/// marque la continuation du mot (`space_after = false`).
fn write_lyrics(out: &mut String, syllables: &[Syllable]) {
    out.push_str("    \\new VaticanaLyrics \\lyricsto \"cantus\" {\n      ");
    let mut first = true;
    for (index, syllable) in syllables.iter().enumerate() {
        let notes = syllable
            .notes
            .iter()
            .filter(|note| note.letter != '\0')
            .count();
        for note_index in 0..notes {
            if !first {
                if note_index == 0 && index > 0 {
                    let previous = &syllables[index - 1];
                    if !previous.space_after && !previous.text.is_empty() {
                        out.push_str(" -- ");
                    } else {
                        out.push(' ');
                    }
                } else {
                    out.push(' ');
                }
            }
            // Une chaîne (même vide) ouvre une syllabe : un groupe sans texte
            // est un fait GABC, pas un mélisme sur la syllabe précédente.
            if note_index == 0 {
                let _ = write!(out, "\"{}\"", escape(&syllable.text));
            } else {
                out.push('_');
            }
            first = false;
        }
    }
    out.push_str("\n    }\n");
}

/// Clé GABC `c3`/`cb3`/`f3` → nom de clé LilyPond (§17.4.2). Les clefs
/// au-delà de la gamme vaticane utilisent le nom de clé ordinaire, lui aussi
/// standard (§"Clef").
fn lily_clef(token: &str) -> String {
    let first = token.split('@').next().unwrap_or(token);
    let flat = first.contains('b');
    let line = first
        .chars()
        .last()
        .and_then(|c| c.to_digit(10))
        .unwrap_or(4);
    let family = first.chars().next().unwrap_or('c');
    if flat {
        return format!("{family}{line}");
    }
    match (family, line) {
        ('c', 1..=3) => format!("vaticana-do{line}"),
        ('f', 1..=2) => format!("vaticana-fa{line}"),
        _ => format!("{family}{line}"),
    }
}

/// Inverse de `scoretk_gabc_common::syntax::diatonic_step` : hauteur
/// diatonique (C4 = 0) → hauteur écrite LilyPond.
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

fn lily_bar(bar: BarKind) -> &'static str {
    match bar {
        BarKind::Virgula | BarKind::VirgulaLedger => "\\caesura",
        BarKind::DivisioMinimis | BarKind::DivisioMinima | BarKind::DivisioMinimaLedger => {
            "\\divisioMinima"
        }
        // §17.4.4 ne définit ni `divisioMinor` ni de variante chiffrée :
        // la barre GABC `;` (demi-barre) prend le glyphe moyen, `:` le long.
        BarKind::DivisioMinor | BarKind::Dominican(_) => "\\divisioMaior",
        BarKind::DivisioMaior | BarKind::DivisioMaiorDot => "\\divisioMaxima",
        BarKind::DivisioFinalis => "\\finalis",
    }
}

fn escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            _ => result.push(character),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gregorian_uses_vaticana_contexts_and_divisiones() {
        let gregorian = Gregorian {
            clefs: vec!["c3".to_owned()],
            syllables: vec![Syllable {
                text: "San--ctus".to_owned(),
                notes: vec![GregorianNote {
                    letter: 'e',
                    glyph: "w".to_owned(),
                    signs: vec![scoretk_core::model::NeumeSign::new(SignKind::Quilisma, "w")],
                    ..GregorianNote::default()
                }],
                bars: vec![BarKind::DivisioMinima],
                ..Syllable::default()
            }],
            ..Gregorian::default()
        };
        let score = Score::default();
        let ly = render(&score, &gregorian, "2.24.0");
        assert!(ly.contains("\\new VaticanaVoice"));
        assert!(ly.contains("\\clef \"vaticana-do3\""));
        assert!(ly.contains("\\quilisma"));
        assert!(ly.contains("\\divisioMinima"));
        assert!(ly.contains("Custos_engraver"));
        assert!(ly.contains("\"San--ctus\""));
    }
}
