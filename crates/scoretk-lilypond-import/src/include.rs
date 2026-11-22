// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage du traitement de `\include` de `lily/lexer.ll` de LilyPond,
// Copyright (C) 1998--2026 Han-Wen Nienhuys et al., modifié (réécrit en Rust)
// en 2026.
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

//! Traitement de `\include` avant l'analyse syntaxique.
//!
//! LilyPond inclut le fichier demandé dans le flux du lexer
//! (`lily/lexer.ll` : la commande empile un fichier et la lecture continue).
//! Ici, l'inclusion est faite sur les jetons : le `\include` et son argument
//! sont remplacés par les jetons du fichier inclus, au niveau supérieur comme
//! au milieu de la musique. Aucune entrée/sortie n'a lieu dans ce module : la
//! lecture des fichiers passe par le rappel `resolve` fourni par l'appelant,
//! ce qui garde la bibliothèque portable.
//!
//! Deux familles de `\include` ne demandent pas de fichier :
//!
//! - les fichiers de langue livrés par LilyPond (`italiano.ly`,
//!   `english.ly`…) valent `\language "<langue>"` ;
//! - les fichiers livrés avec LilyPond qui ne portent pas de musique sont
//!   ignorés sans avertissement (liste [`FICHIERS_LILYPOND`]).

use std::collections::HashMap;

use crate::lexer::{tokenize, Kind, Token};
use crate::pitch::Language;
use crate::warnings::Warnings;

/// Profondeur maximale d'inclusion imbriquée.
const PROFONDEUR_MAX: usize = 32;

/// Fichiers livrés dans `ly/` avec LilyPond, sans musique utile à la lecture.
///
/// Ils définissent des contextes, des gravures, des articulations MIDI ou des
/// gammes hors du sous-ensemble lu. Les inclure ne doit produire ni
/// avertissement ni tentative de lecture sur le disque.
const FICHIERS_LILYPOND: &[&str] = &[
    // Préambules et moteurs livrés avec LilyPond.
    "lilypond-book-preamble.ly",
    "articulate.ly",
    "gregorian.ly",
    "event-listener.ly",
    // Initialisations de gravure et de contextes.
    "ancient-init.ly",
    "chord-modifiers-init.ly",
    "drumpitch-init.ly",
    "midi-init.ly",
    "scale-definitions-init.ly",
    "script-init.ly",
    "string-tunings-init.ly",
    // Gammes et modes hors du sous-ensemble (makam, maqam, percussions).
    "arabic.ly",
    "bagpipe.ly",
    "hel-arabic.ly",
    "makam.ly",
    "persian.ly",
    "turkish-makam.ly",
    // Aides à l'écriture (chœurs, accords de guitare, swing).
    "predefined-fretboards-init.ly",
    "predefined-guitar-fretboards.ly",
    "predefined-guitar-ninth-fretboards.ly",
    "predefined-mandolin-fretboards.ly",
    "predefined-ukulele-fretboards.ly",
    "satb.ly",
    "ssaattbb.ly",
    "swing.ly",
];

/// Remplace les `\include` d'une source par leurs jetons.
///
/// `resolve(chemin_demandé, chemin_du_fichier_qui_inclut)` renvoie le texte du
/// fichier demandé ou `None` s'il est introuvable ; le second argument vaut
/// `None` au niveau supérieur, puis le chemin du fichier inclus (normalisé,
/// relatif au fichier principal). Un fichier inclus autre qu'un fichier de
/// langue ou de la liste [`FICHIERS_LILYPOND`] que le résolveur ne trouve pas
/// est ignoré avec l'avertissement habituel.
pub(crate) fn expand(
    source: &str,
    resolve: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
    warnings: &mut Warnings,
) -> Vec<Token> {
    let mut expander = Expander {
        resolve,
        warnings,
        tokens: Vec::new(),
        stack: Vec::new(),
        strings: HashMap::new(),
    };
    expander.source(source, None, 0);
    expander.tokens
}

/// État de l'expansion des `\include` d'une source.
struct Expander<'a> {
    /// Rappel de lecture des fichiers inclus.
    resolve: &'a mut dyn FnMut(&str, Option<&str>) -> Option<String>,
    /// Avertissements collectés.
    warnings: &'a mut Warnings,
    /// Jetons produits.
    tokens: Vec<Token>,
    /// Chemins des fichiers en cours d'inclusion, pour détecter les cycles.
    stack: Vec<String>,
    /// Identifiants assignés à une chaîne (`nom = "fichier.ly"`), pour
    /// `\include \nom` (`lily/lexer.ll:340-350`).
    strings: HashMap<String, String>,
}

impl Expander<'_> {
    /// Étend les `\include` d'une source dans les jetons.
    ///
    /// `current` est le chemin du fichier en cours (relatif au fichier
    /// principal), `depth` sa profondeur d'inclusion.
    fn source(&mut self, source: &str, current: Option<&str>, depth: usize) {
        let source_tokens = tokenize(source);
        let mut index = 0usize;
        while let Some(token) = source_tokens.get(index) {
            // `#(ly:parser-include-string "…")` inclut son argument comme un
            // fichier (`lily/lexer.ll`) : on le relit récursivement, avec le
            // même garde-fou de profondeur que les `\include`.
            if token.kind == Kind::Scheme {
                if let Some(injected) = parser_include_string(&token.text) {
                    let (line, column) = (token.line, token.column);
                    if depth >= PROFONDEUR_MAX {
                        self.warnings.push(
                            line,
                            column,
                            "ly:parser-include-string ignoré : profondeur maximale dépassée",
                        );
                    } else {
                        self.source(&injected, current, depth + 1);
                    }
                    index += 1;
                    continue;
                }
            }
            // `nom = "chaîne"` : retenu pour un `\include \nom` plus loin.
            if let (
                Some(Token {
                    kind: Kind::Word,
                    text: nom,
                    ..
                }),
                Some(Token {
                    kind: Kind::Punct('='),
                    ..
                }),
                Some(Token {
                    kind: Kind::Str,
                    text: valeur,
                    ..
                }),
            ) = (
                source_tokens.get(index),
                source_tokens.get(index + 1),
                source_tokens.get(index + 2),
            ) {
                self.strings.insert(nom.clone(), valeur.clone());
            }
            if !token.is_command("include") {
                self.tokens.push(token.clone());
                index += 1;
                continue;
            }
            let (line, column) = (token.line, token.column);
            let ignored = "\\include ignoré : le fichier inclus n'est pas lu";
            // `lily/lexer.ll:333-350` : l'argument est une chaîne littérale ou
            // un identifiant dont la valeur est une chaîne.
            let demande = match source_tokens.get(index + 1) {
                Some(Token {
                    kind: Kind::Str,
                    text,
                    ..
                }) => Some(text.clone()),
                Some(Token {
                    kind: Kind::Command,
                    text,
                    ..
                }) => self.strings.get(text).cloned(),
                Some(Token {
                    kind: Kind::Scheme, ..
                }) => {
                    self.warnings.push(
                        line,
                        column,
                        "\\include d'une expression Scheme ignoré : la valeur n'est pas connue",
                    );
                    self.warnings.mark_music_ignored();
                    None
                }
                _ => {
                    self.warnings.push(line, column, ignored);
                    self.warnings.mark_music_ignored();
                    None
                }
            };
            if let Some(text) = demande {
                if let Some(language) = language_argument(&text) {
                    // `\include "italiano.ly"` vaut `\language "italiano"`.
                    self.tokens.push(Token {
                        kind: Kind::Command,
                        text: "language".to_owned(),
                        line,
                        column,
                    });
                    self.tokens.push(Token {
                        kind: Kind::Str,
                        text: language.to_owned(),
                        line,
                        column,
                    });
                } else if FICHIERS_LILYPOND.contains(&text.as_str()) {
                    // Un fichier livré par LilyPond peut porter la structure
                    // du `\score` (`satb.ly`, `ssaattbb.ly`) : l'ignorer ne
                    // doit pas faire échouer le fichier.
                    self.warnings.mark_music_ignored();
                } else {
                    self.included(&text, current, depth, line, column);
                }
            }
            // La commande et son argument sont consommés, même ignorés.
            index += 2;
        }
    }

    /// Inclut un fichier par le résolveur, avec les garde-fous de profondeur et
    /// de cycle.
    fn included(
        &mut self,
        requested: &str,
        current: Option<&str>,
        depth: usize,
        line: u32,
        column: u32,
    ) {
        if depth >= PROFONDEUR_MAX {
            self.warnings.push(
                line,
                column,
                format!(
                    "\\include ignoré : profondeur d'inclusion maximale dépassée ({PROFONDEUR_MAX})"
                ),
            );
            return;
        }
        let path = join_path(current, requested);
        if self.stack.contains(&path) {
            self.warnings.push(
                line,
                column,
                format!("\\include cyclique ignoré : {requested}"),
            );
            return;
        }
        match (self.resolve)(requested, current) {
            Some(source) => {
                self.stack.push(path.clone());
                self.source(&source, Some(&path), depth + 1);
                self.stack.pop();
            }
            None => {
                self.warnings.push(
                    line,
                    column,
                    "\\include ignoré : le fichier inclus n'est pas lu",
                );
                self.warnings.mark_music_ignored();
            }
        }
    }
}

/// Argument texte d'un `#(ly:parser-include-string "…")`.
///
/// Le lexer lit toute l'expression Scheme en un seul jeton. On reconnaît
/// l'appel précis (et non une expression quelconque contenant son nom) et on
/// renvoie le contenu de sa chaîne, tel qu'il sera relu par le lexer.
fn parser_include_string(datum: &str) -> Option<String> {
    const APPEL: &str = "(ly:parser-include-string";
    let reste = datum.trim_start().strip_prefix(APPEL)?.trim_start();
    let reste = reste.strip_prefix('"')?;
    let fin = reste.rfind('"')?;
    if fin == 0 {
        return None;
    }
    Some(decode_scheme_escapes(&reste[..fin]))
}

/// Décode les échappements d'une chaîne Scheme (`lily/lily-lexer.cc:302-315`
/// `escaped_char`) : `\\` vaut un seul backslash, `\"` une guillemet, `\n` une
/// newline et `\t` une tabulation. Sans cela `\\relative` reste deux
/// backslashes et la commande n'est pas reconnue.
fn decode_scheme_escapes(texte: &str) -> String {
    let mut decode = String::with_capacity(texte.len());
    let mut caracteres = texte.chars();
    while let Some(caractere) = caracteres.next() {
        if caractere != '\\' {
            decode.push(caractere);
            continue;
        }
        match caracteres.next() {
            Some('n') => decode.push('\n'),
            Some('t') => decode.push('\t'),
            Some('r') => decode.push('\r'),
            Some(suivant @ ('\\' | '"' | '\'')) => decode.push(suivant),
            Some(suivant) => {
                decode.push('\\');
                decode.push(suivant);
            }
            None => decode.push('\\'),
        }
    }
    decode
}

/// Langue d'un fichier de langue livré par LilyPond.
///
/// Renvoie le nom de langue à donner à `\language` (`italiano` pour
/// `italiano.ly`), ou `None` si le fichier n'est pas un fichier de langue.
fn language_argument(file: &str) -> Option<&str> {
    let stem = file.strip_suffix(".ly")?;
    if stem.contains('/') {
        return None;
    }
    Language::from_name(stem).map(|_| stem)
}

/// Chemin d'un fichier inclus, relatif au fichier principal.
///
/// Le dossier du fichier qui inclut est joint au chemin demandé, puis les `.`
/// et `..` sont résolus sans sortir de la racine.
fn join_path(current: Option<&str>, requested: &str) -> String {
    match current.and_then(|path| path.rsplit_once('/').map(|(dir, _)| dir)) {
        Some(dir) => normalize_path(&format!("{dir}/{requested}")),
        None => normalize_path(requested),
    }
}

/// Normalise un chemin relatif : retire `.` et résout `..` lexicalement.
fn normalize_path(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let normalized = parts.join("/");
    if absolute {
        format!("/{normalized}")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Étend une source sans aucun fichier joignable.
    fn texts(source: &str) -> Vec<String> {
        let mut warnings = Warnings::new();
        let mut resolve = |_: &str, _: Option<&str>| None;
        expand(source, &mut resolve, &mut warnings)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Étend une source avec une table de fichiers en mémoire.
    fn texts_avec(source: &str, files: &HashMap<&str, &str>) -> (Vec<String>, Vec<crate::Warning>) {
        let mut warnings = Warnings::new();
        let mut resolve =
            |name: &str, _: Option<&str>| files.get(name).map(|text| (*text).to_owned());
        let tokens = expand(source, &mut resolve, &mut warnings);
        (
            tokens.iter().map(ToString::to_string).collect(),
            warnings.into_vec(),
        )
    }

    #[test]
    fn fichier_de_langue_devient_language() {
        assert_eq!(
            texts("\\include \"italiano.ly\" do4"),
            vec!["\\language", "\"italiano\"", "do", "4"]
        );
        assert_eq!(
            texts("\\include \"catalan.ly\""),
            vec!["\\language", "\"catalan\""]
        );
    }

    #[test]
    fn fichiers_livres_ignores_sans_avertissement() {
        let mut warnings = Warnings::new();
        let mut resolve = |_: &str, _: Option<&str>| None;
        let tokens = expand(
            "\\include \"articulate.ly\" \\include \"gregorian.ly\" c4",
            &mut resolve,
            &mut warnings,
        );
        assert_eq!(
            tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["c", "4"]
        );
        assert!(warnings.into_vec().is_empty());
    }

    #[test]
    fn fichier_ordinaire_averti_sans_resolveur() {
        let mut warnings = Warnings::new();
        let mut resolve = |_: &str, _: Option<&str>| None;
        let tokens = expand("\\include \"macros.ly\" c4", &mut resolve, &mut warnings);
        assert_eq!(
            tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["c", "4"]
        );
        let warnings = warnings.into_vec();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].message.contains("\\include ignoré"),
            "{warnings:?}"
        );
        assert_eq!((warnings[0].line, warnings[0].column), (1, 1));
    }

    #[test]
    fn fichier_inclus_remplace_la_commande() {
        let files = HashMap::from([("macros.ly", "m = { c4 }")]);
        let (tokens, warnings) = texts_avec("\\include \"macros.ly\" d4", &files);
        assert_eq!(tokens, vec!["m", "=", "{", "c", "4", "}", "d", "4"]);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn chemin_du_fichier_qui_inclut() {
        let mut files: HashMap<&str, &str> = HashMap::new();
        files.insert("sub/a.ly", "\\include \"b.ly\"");
        files.insert("b.ly", "d4");
        let mut appels = Vec::new();
        let mut warnings = Warnings::new();
        let mut resolve = |name: &str, includer: Option<&str>| {
            appels.push((name.to_owned(), includer.map(str::to_owned)));
            files.get(name).map(|text| (*text).to_owned())
        };
        let tokens = expand("\\include \"sub/a.ly\"", &mut resolve, &mut warnings);
        assert_eq!(
            tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["d", "4"]
        );
        assert_eq!(
            appels,
            vec![
                ("sub/a.ly".to_owned(), None),
                ("b.ly".to_owned(), Some("sub/a.ly".to_owned()))
            ]
        );
    }

    #[test]
    fn cycle_detecte() {
        let files = HashMap::from([
            ("a.ly", "\\include \"b.ly\""),
            ("b.ly", "\\include \"a.ly\""),
        ]);
        let (tokens, warnings) = texts_avec("\\include \"a.ly\" c4", &files);
        assert_eq!(tokens, vec!["c", "4"]);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.message.contains("cyclique")),
            "{warnings:?}"
        );
    }

    #[test]
    fn profondeur_limitee() {
        // Chaque fichier inclut le suivant, sans cycle : l'expansion s'arrête à
        // la profondeur maximale.
        let mut files: HashMap<String, String> = HashMap::new();
        for index in 0..PROFONDEUR_MAX + 5 {
            files.insert(
                format!("{index}.ly"),
                format!("\\include \"{}.ly\"", index + 1),
            );
        }
        let mut warnings = Warnings::new();
        let mut resolve = |name: &str, _: Option<&str>| files.get(name).cloned();
        let tokens = expand("\\include \"0.ly\"", &mut resolve, &mut warnings);
        assert!(tokens.is_empty());
        assert!(
            warnings
                .into_vec()
                .iter()
                .any(|warning| warning.message.contains("profondeur")),
            "profondeur non signalée"
        );
    }

    #[test]
    fn include_scheme_averti() {
        let mut warnings = Warnings::new();
        let mut resolve = |_: &str, _: Option<&str>| None;
        let tokens = expand("\\include #fichier c4", &mut resolve, &mut warnings);
        assert_eq!(
            tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["c", "4"]
        );
        let warnings = warnings.into_vec();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].message.contains("Scheme"), "{warnings:?}");
    }
}
