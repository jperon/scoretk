// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/musicxml/internal/export/exportmusicxml.cpp` et
// `engraving/dom/chordlist.cpp` de MuseScore Studio, Copyright (C) MuseScore
// Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Accords chiffrés (`harmony`).
//!
//! Portage de `ExportMusicXml::harmony` (`exportmusicxml.cpp`) et de la partie
//! utile de `ParsedChord::parse` (`engraving/dom/chordlist.cpp`). MuseScore
//! analyse le suffixe écrit dans `<name>` pour retrouver le `<kind>` MusicXML,
//! ses degrés et son texte ; le modèle ne garde que le nom brut et les degrés
//! explicites, d'où cette analyse à l'export.
//!
//! L'analyse est volontairement limitée aux qualités et extensions courantes :
//! une qualité inconnue produit `<kind text="…">none</kind>`, comme MuseScore
//! pour un accord non reconnu.

use crate::model::{Harmony, HarmonyDegreeKind, Placement, Tick};

use super::exportmusicxml::Out;
use super::pitch;

/// Écrit un accord chiffré.
///
/// `offset` est la position de l'accord par rapport au début du segment qui le
/// porte, quand MuseScore le rattache à un accord ou un silence précédent.
pub(crate) fn write(out: &mut Out, harmony: &Harmony, offset: Tick) {
    out.attr(false);
    let mut attributes: Vec<(&str, String)> = Vec::new();
    if harmony.placement == Placement::Below {
        attributes.push(("placement", "below".to_owned()));
    }
    attributes.push(("print-frame", "no".to_owned()));
    let borrowed: Vec<(&str, &str)> = attributes
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    out.xml.start_element("harmony", &borrowed);

    match harmony.kind.as_str() {
        "roman" => write_roman(out, harmony),
        "nashville" => write_nashville(out, harmony),
        _ => write_standard(out, harmony),
    }

    if offset > Tick::ZERO {
        let duration = out.duration_in_divisions(offset);
        if duration > 0 {
            out.xml.tag("offset", &duration.to_string());
        }
    }

    out.xml.end_element();
}

/// Branche `HarmonyType::STANDARD` : fondamentale, qualité et degrés.
fn write_standard(out: &mut Out, harmony: &Harmony) {
    if pitch::tpc_is_valid(harmony.root_tpc) {
        write_step_alter(out, "root", harmony.root_tpc);
        let parsed = parse(&harmony.name);
        match parsed {
            Some(parsed) if !parsed.kind.is_empty() => {
                write_kind(out, &parsed);
                if let Some(bass) = harmony.bass_tpc {
                    if pitch::tpc_is_valid(bass) {
                        out.xml
                            .start_element("bass", &[("arrangement", "horizontal")]);
                        out.xml
                            .tag("bass-step", &pitch::tpc_step_name(bass).to_string());
                        let alter = pitch::tpc_alter(bass);
                        if alter != 0 {
                            out.xml.tag("bass-alter", &alter.to_string());
                        }
                        out.xml.end_element();
                    }
                }
                for degree in &parsed.degrees {
                    write_degree(out, degree);
                }
                for degree in &harmony.degrees {
                    write_explicit_degree(out, degree);
                }
            }
            _ => write_unrecognized(out, harmony),
        }
    } else {
        // Accord non reconnu : MuseScore écrit une fondamentale « C » sans
        // texte, puis le nom brut comme `<kind>`.
        write_root_placeholder(out, &harmony.name);
    }
}

/// Écrit un accord non analysable sous une fondamentale valide.
///
/// MuseScore n'écrit alors ni basse ni degré, seulement le nom brut.
fn write_unrecognized(out: &mut Out, harmony: &Harmony) {
    if harmony.name.is_empty() {
        out.xml.tag("kind", "none");
    } else {
        out.xml
            .tag_with("kind", &[("text", harmony.name.as_str())], "none");
    }
}

/// Branche `HarmonyType::ROMAN` : chiffrage en degrés.
fn write_roman(out: &mut Out, harmony: &Harmony) {
    let text = if harmony.name.is_empty() {
        harmony.text.as_str()
    } else {
        harmony.name.as_str()
    };
    let Some((alter, number)) = roman_numeral(text) else {
        // MuseScore retombe sur la branche standard : fondamentale « C » sans
        // texte puis le nom brut.
        write_root_placeholder(out, text);
        return;
    };

    out.xml.start_element("numeral", &[]);
    out.xml
        .tag_with("numeral-root", &[("text", number)], &roman_value(number));
    if alter != 0 {
        out.xml.tag("numeral-alter", &alter.to_string());
    }
    out.xml.end_element();
    let major = number.starts_with(|c: char| c.is_ascii_uppercase());
    out.xml.tag("kind", if major { "major" } else { "minor" });
    if text.ends_with("64") {
        out.xml.tag("inversion", "2");
    } else if text.ends_with('6') {
        out.xml.tag("inversion", "1");
    }
}

/// Branche `HarmonyType::NASHVILLE` : fonction chiffrée.
fn write_nashville(out: &mut Out, harmony: &Harmony) {
    let text = if harmony.name.is_empty() {
        harmony.text.as_str()
    } else {
        harmony.name.as_str()
    };
    let Some(function) = harmony
        .function
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        // Fonction absente : MuseScore écrit le nom brut comme `<function>`
        // obsolète, puis `kind` valant `none`.
        out.xml.tag("function", text);
        out.xml.tag("kind", "none");
        return;
    };

    let (alter, function) = match function.strip_prefix('b') {
        Some(rest) => (-1, rest),
        None => match function.strip_prefix('#') {
            Some(rest) => (1, rest),
            None => (0, function),
        },
    };
    out.xml.start_element("numeral", &[]);
    out.xml.tag("numeral-root", function);
    if alter != 0 {
        out.xml.tag("numeral-alter", &alter.to_string());
    }
    out.xml.end_element();

    match parse(text) {
        Some(parsed) if !parsed.kind.is_empty() => write_kind(out, &parsed),
        _ => out.xml.tag("kind", "major"),
    }
}

/// Écrit `<kind>`, avec ses attributs de texte et de parenthèses.
fn write_kind(out: &mut Out, parsed: &Parsed) {
    let mut attributes: Vec<(&str, String)> = Vec::new();
    if !parsed.text.is_empty() {
        attributes.push(("text", parsed.text.clone()));
    }
    if parsed.symbols {
        attributes.push(("use-symbols", "yes".to_owned()));
    }
    if parsed.parens {
        attributes.push(("parentheses-degrees", "yes".to_owned()));
    }
    let borrowed: Vec<(&str, &str)> = attributes
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    out.xml.tag_with("kind", &borrowed, &parsed.kind);
}

/// Écrit un degré analysé (`add9`, `add#7`, `sub5`…).
fn write_degree(out: &mut Out, degree: &str) {
    let (kind, rest) = if let Some(rest) = degree.strip_prefix("add") {
        ("add", rest)
    } else if let Some(rest) = degree.strip_prefix("sub") {
        ("subtract", rest)
    } else if let Some(rest) = degree.strip_prefix("alt") {
        ("alter", rest)
    } else {
        return;
    };
    let (alter, value) = if let Some(rest) = rest.strip_prefix('#') {
        (1, rest)
    } else {
        match rest.strip_prefix('b') {
            Some(rest) => (-1, rest),
            None => (0, rest),
        }
    };
    out.xml.start_element("degree", &[]);
    out.xml.tag("degree-value", value);
    out.xml.tag("degree-alter", &alter.to_string());
    out.xml.tag("degree-type", kind);
    out.xml.end_element();
}

/// Écrit un degré explicite du modèle (`<degree>`).
fn write_explicit_degree(out: &mut Out, degree: &crate::model::HarmonyDegree) {
    let kind = match degree.kind {
        HarmonyDegreeKind::Add => "add",
        HarmonyDegreeKind::Alter => "alter",
        HarmonyDegreeKind::Subtract => "subtract",
    };
    out.xml.start_element("degree", &[]);
    out.xml.tag("degree-value", &degree.value.to_string());
    out.xml.tag("degree-alter", &degree.alter.to_string());
    out.xml.tag("degree-type", kind);
    out.xml.end_element();
}

/// Écrit `<nom>` avec `step` et `alter` (fondamentale ou basse).
fn write_step_alter(out: &mut Out, name: &str, tpc: i8) {
    out.xml.start_element(name, &[]);
    out.xml.tag(&format!("{name}-step"), &step_letters(tpc));
    let alter = pitch::tpc_alter(tpc);
    if alter != 0 {
        out.xml.tag(&format!("{name}-alter"), &alter.to_string());
    }
    out.xml.end_element();
}

/// Écrit une fondamentale « C » sans texte, comme un accord non reconnu.
fn write_root_placeholder(out: &mut Out, text: &str) {
    out.xml.start_element("root", &[]);
    out.xml.tag_with("root-step", &[("text", "")], "C");
    out.xml.end_element();
    if text.is_empty() {
        out.xml.tag("kind", "none");
    } else {
        out.xml.tag_with("kind", &[("text", text)], "none");
    }
}

/// Nom du degré d'un `tpc` (`tpc2stepName`).
fn step_letters(tpc: i8) -> String {
    pitch::tpc_step_name(tpc).to_string()
}

/// Valeur numérique d'un chiffre romain (`numeral-root`, `tpc2Function`).
///
/// Reproduit la branche romaine de `ExportMusicXml::harmony` : les lettres
/// `i` et `v` comptent, et un chiffre commençant par `i` vaut 4 quand il
/// contient un `v` (donc `iv` vaut 4 et non 6).
fn roman_value(number: &str) -> String {
    let lower = number.to_lowercase();
    let value = if lower.contains('v') {
        if lower.starts_with('i') {
            4
        } else {
            4 + number.chars().count()
        }
    } else {
        number.chars().count()
    };
    value.to_string()
}

/// Analyse `(b|#)?([ivIV]+)` en altération et chiffre romain.
fn roman_numeral(text: &str) -> Option<(i32, &str)> {
    let (alter, rest) = match text.strip_prefix('b') {
        Some(rest) => (-1, rest),
        None => match text.strip_prefix('#') {
            Some(rest) => (1, rest),
            None => (0, text),
        },
    };
    if rest.is_empty() || !rest.chars().all(|c| matches!(c, 'i' | 'v' | 'I' | 'V')) {
        return None;
    }
    Some((alter, rest))
}

/// Résultat de l'analyse d'un suffixe d'accord (`ParsedChord`).
struct Parsed {
    /// Valeur MusicXML de `<kind>` (`minor`, `dominant`…), vide si inconnue.
    kind: String,
    /// Texte écrit dans l'attribut `text` de `<kind>`.
    text: String,
    /// Vrai quand le nom utilise les symboles de qualité (`+`, `°`, `-`…).
    symbols: bool,
    /// Vrai quand un degré est entre parenthèses.
    parens: bool,
    /// Degrés ajoutés (`add9`, `add#7`, `sub5`…).
    degrees: Vec<String>,
}

/// Analyse le suffixe d'un accord, comme `ParsedChord::parse`.
///
/// Renvoie `None` quand la qualité n'est pas reconnue : MuseScore écrit alors
/// `<kind text="…">none</kind>`.
fn parse(name: &str) -> Option<Parsed> {
    let mut parsed = Parsed {
        kind: String::new(),
        text: String::new(),
        symbols: false,
        parens: false,
        degrees: Vec::new(),
    };

    let bytes = name.as_bytes();
    let mut i = 0;
    // Qualité : lettres initiales jusqu'au premier chiffre, parenthèse,
    // virgule, barre oblique ou espace.
    while i < bytes.len() && !bytes[i].is_ascii_digit() && !is_separator(bytes[i]) {
        i += 1;
    }
    let token = &name[..i];
    let (quality, consumed) = split_quality(token);
    i = consumed;
    if quality.is_none() && !token.is_empty() {
        return None;
    }
    let quality = quality.unwrap_or(Default::default());
    parsed.kind = quality.name.clone();
    parsed.text = quality.token.clone();
    parsed.symbols = quality.symbols;

    // Extension : chiffres, virgules et barres obliques.
    let start = i;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b',' || bytes[i] == b'/') {
        i += 1;
    }
    let extension = &name[start..i];
    let (mut take6, mut take7, mut take9, mut take11, mut take13) = quality.takes;
    let mut kind = quality.name.clone();
    if kind.is_empty() {
        // Sans qualité explicite, l'extension décide : `7`, `9`, `11` et `13`
        // donnent un accord de dominante, le reste un accord majeur.
        if matches!(extension, "7" | "9" | "11" | "13") {
            kind = "dominant".to_owned();
            take7 = true;
            take9 = true;
            take11 = true;
            take13 = true;
        } else {
            kind = "major".to_owned();
            take6 = true;
            take7 = true;
            take9 = true;
            take11 = true;
            take13 = true;
        }
        parsed.kind = kind.clone();
    }
    let mut text = parsed.text.clone();
    text.push_str(extension);
    match extension {
        "5" => parsed.kind = "power".to_owned(),
        "6" if take6 => parsed.kind.push_str("-sixth"),
        "7" if take7 => parsed.kind.push_str("-seventh"),
        "9" if take9 => parsed.kind.push_str("-ninth"),
        "11" if take11 => parsed.kind.push_str("-11th"),
        "13" if take13 => parsed.kind.push_str("-13th"),
        "9" if take7 => {
            parsed.kind.push_str("-seventh");
            parsed.degrees.push("add9".to_owned());
            text = remove_extension(&text, extension);
        }
        "11" if take7 => {
            parsed.kind.push_str("-seventh");
            parsed.degrees.push("add9".to_owned());
            parsed.degrees.push("add11".to_owned());
            text = remove_extension(&text, extension);
        }
        "13" if take7 => {
            parsed.kind.push_str("-seventh");
            parsed.degrees.push("add9".to_owned());
            parsed.degrees.push("add11".to_owned());
            parsed.degrees.push("add13".to_owned());
            text = remove_extension(&text, extension);
        }
        "2" => {
            parsed.degrees.push("add2".to_owned());
            text = remove_extension(&text, extension);
        }
        "4" => {
            parsed.degrees.push("add4".to_owned());
            text = remove_extension(&text, extension);
        }
        _ => {}
    }
    if parsed.kind == "dominant-seventh" {
        parsed.kind = "dominant".to_owned();
    }
    parsed.text = text;

    // Modificateurs : parenthèses, `addN`, `susN`, `noN`/`omitN`, `bN`/`#N`.
    let mut pending_add = false;
    let mut pending_sus = false;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => {
                parsed.parens = true;
                i += 1;
                continue;
            }
            b')' | b',' | b'/' | b'\\' | b' ' => {
                i += 1;
                continue;
            }
            _ => {}
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_digit() && !is_separator(bytes[i]) {
            i += 1;
        }
        let token = &name[start..i];
        let lower = token.to_lowercase();
        match lower.as_str() {
            "add" => {
                pending_add = true;
                continue;
            }
            "sus" if i < bytes.len() => {
                pending_sus = true;
                continue;
            }
            _ => {}
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let number = &name[start..i];
        if pending_add {
            let (alter, value) = alteration(token, number);
            if !value.is_empty() {
                parsed.degrees.push(format!("add{alter}{value}"));
            }
            pending_add = false;
        } else if pending_sus {
            let (alter, value) = alteration(token, number);
            let value = if value.is_empty() { "4" } else { value };
            parsed.kind = match value {
                "2" => "suspended-second".to_owned(),
                _ => "suspended-fourth".to_owned(),
            };
            parsed.text = format!("sus{alter}{value}");
            pending_sus = false;
        } else if lower == "no" || lower == "omit" {
            if !number.is_empty() {
                parsed.degrees.push(format!("sub{number}"));
            }
        } else if lower == "sus" {
            parsed.kind = "suspended-fourth".to_owned();
            parsed.text = "sus4".to_owned();
        } else if !number.is_empty() {
            let (alter, value) = alteration(token, number);
            parsed.degrees.push(format!("add{alter}{value}"));
        }
    }

    Some(parsed)
}

/// Qualité reconnue en tête d'un suffixe.
#[derive(Default)]
struct Quality {
    /// Valeur MusicXML (`minor`, `major`, `diminished`…).
    name: String,
    /// Texte écrit dans `<kind text>`.
    token: String,
    /// Vrai pour les qualités symboliques (`+`, `°`, `-`).
    symbols: bool,
    /// Extensions acceptées telles quelles (`take6`, `take7`, …).
    takes: (bool, bool, bool, bool, bool),
}

/// Reconnaît la qualité initiale, en renvoyant la longueur consommée.
fn split_quality(token: &str) -> (Option<Quality>, usize) {
    const MAJOR: [&str; 6] = ["major", "maj", "ma", "M", "^", "t"];
    const MINOR: [&str; 5] = ["minor", "min", "mi", "m", "-"];
    const DIMINISHED: [&str; 4] = ["dim.", "dim", "o", "°"];
    const AUGMENTED: [&str; 2] = ["aug", "+"];
    let all = (true, true, true, true, true);
    for alias in MAJOR {
        if let Some(rest) = token.strip_prefix(alias) {
            return (
                Some(Quality {
                    name: "major".to_owned(),
                    token: alias.to_owned(),
                    symbols: matches!(alias, "^" | "t"),
                    takes: all,
                }),
                token.len() - rest.len(),
            );
        }
    }
    for alias in MINOR {
        if let Some(rest) = token.strip_prefix(alias) {
            return (
                Some(Quality {
                    name: "minor".to_owned(),
                    token: alias.to_owned(),
                    symbols: alias == "-",
                    takes: all,
                }),
                token.len() - rest.len(),
            );
        }
    }
    for alias in DIMINISHED {
        if let Some(rest) = token.strip_prefix(alias) {
            return (
                Some(Quality {
                    name: "diminished".to_owned(),
                    token: alias.to_owned(),
                    symbols: matches!(alias, "o" | "°"),
                    takes: (false, true, false, false, false),
                }),
                token.len() - rest.len(),
            );
        }
    }
    for alias in AUGMENTED {
        if let Some(rest) = token.strip_prefix(alias) {
            return (
                Some(Quality {
                    name: "augmented".to_owned(),
                    token: alias.to_owned(),
                    symbols: alias == "+",
                    takes: (false, true, false, false, false),
                }),
                token.len() - rest.len(),
            );
        }
    }
    if token.starts_with('0') {
        return (
            Some(Quality {
                name: "half-diminished".to_owned(),
                token: "0".to_owned(),
                symbols: true,
                takes: (false, false, false, false, false),
            }),
            1,
        );
    }
    // Pas de qualité explicite : un modificateur initial (`sus4`, `add9`…)
    // est relu depuis le début, comme `ParsedChord::parse`.
    if token.is_empty()
        || ["sus", "add", "no", "omit", "b", "#"]
            .iter()
            .any(|prefix| token.starts_with(prefix))
    {
        return (Some(Quality::default()), 0);
    }
    (None, 0)
}

/// Altération et valeur d'un modificateur (`add`, `sus`, `b`, `#`).
fn alteration<'a>(token: &str, number: &'a str) -> (&'static str, &'a str) {
    match token {
        "b" | "dim" => ("b", number),
        "#" | "aug" | "+" => ("#", number),
        "M" | "maj" | "major" => ("", number),
        _ => ("", number),
    }
}

/// Retire l'extension du texte quand elle devient un degré.
fn remove_extension(text: &str, extension: &str) -> String {
    match text.strip_suffix(extension) {
        Some(rest) => rest.to_owned(),
        None => text.to_owned(),
    }
}

/// Vrai pour les caractères qui terminent une qualité ou un modificateur.
fn is_separator(byte: u8) -> bool {
    matches!(byte, b'(' | b')' | b',' | b'/' | b'\\' | b' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str) -> (String, String, bool, bool, Vec<String>) {
        let parsed = parse(name).expect("accord reconnu");
        (
            parsed.kind,
            parsed.text,
            parsed.symbols,
            parsed.parens,
            parsed.degrees,
        )
    }

    #[test]
    fn analyse_les_qualites_courantes() {
        assert_eq!(kind("").0, "major");
        assert_eq!(kind("m").0, "minor");
        assert_eq!(kind("7").0, "dominant");
        assert_eq!(kind("m7").0, "minor-seventh");
        assert_eq!(
            kind("M7"),
            (
                "major-seventh".to_owned(),
                "M7".to_owned(),
                false,
                false,
                vec![]
            )
        );
        assert_eq!(kind("dim").0, "diminished");
        assert_eq!(kind("aug").0, "augmented");
        assert_eq!(kind("sus4").0, "suspended-fourth");
        assert_eq!(kind("sus2").0, "suspended-second");
        assert_eq!(kind("5").0, "power");
        assert_eq!(kind("9").0, "dominant-ninth");
        assert_eq!(kind("m9").0, "minor-ninth");
    }

    #[test]
    fn analyse_les_parentheses_et_degres() {
        let (kind, text, symbols, parens, degrees) = kind("m(add9)");
        assert_eq!(kind, "minor");
        assert_eq!(text, "m");
        assert!(!symbols);
        assert!(parens);
        assert_eq!(degrees, vec!["add9".to_owned()]);
    }

    #[test]
    fn refuse_un_nom_inconnu() {
        assert!(parse("xyz").is_none());
        assert!(parse("<>&\"").is_none());
    }

    #[test]
    fn chiffre_romain() {
        assert_eq!(roman_numeral("i"), Some((0, "i")));
        assert_eq!(roman_numeral("bIII"), Some((-1, "III")));
        assert_eq!(roman_numeral("xyz"), None);
        assert_eq!(roman_value("i"), "1");
        assert_eq!(roman_value("iv"), "4");
        assert_eq!(roman_value("VII"), "7");
    }
}
