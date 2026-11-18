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
//! lecture des fichiers est laissée à l'appelant.
//!
//! Deux familles de `\include` ne demandent pas de fichier :
//!
//! - les fichiers de langue livrés par LilyPond (`italiano.ly`,
//!   `english.ly`…) valent `\language "<langue>"` ;
//! - les fichiers livrés avec LilyPond qui ne portent pas de musique sont
//!   ignorés sans avertissement (liste [`FICHIERS_LILYPOND`]).

use crate::lexer::{tokenize, Kind, Token};
use crate::pitch::Language;
use crate::warnings::Warnings;

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
/// Sans résolveur, un fichier inclus autre qu'un fichier de langue ou de la
/// liste [`FICHIERS_LILYPOND`] est ignoré avec l'avertissement habituel.
pub(crate) fn expand(source: &str, warnings: &mut Warnings) -> Vec<Token> {
    let mut tokens = Vec::new();
    expand_into(source, warnings, &mut tokens);
    tokens
}

/// Étend les `\include` d'une source dans `tokens`.
fn expand_into(source: &str, warnings: &mut Warnings, tokens: &mut Vec<Token>) {
    let source_tokens = tokenize(source);
    let mut index = 0usize;
    while let Some(token) = source_tokens.get(index) {
        if !token.is_command("include") {
            tokens.push(token.clone());
            index += 1;
            continue;
        }
        let (line, column) = (token.line, token.column);
        let ignored = "\\include ignoré : le fichier inclus n'est pas lu";
        match source_tokens.get(index + 1) {
            Some(Token {
                kind: Kind::Str,
                text,
                ..
            }) => {
                if let Some(language) = language_argument(text) {
                    // `\include "italiano.ly"` vaut `\language "italiano"`.
                    tokens.push(Token {
                        kind: Kind::Command,
                        text: "language".to_owned(),
                        line,
                        column,
                    });
                    tokens.push(Token {
                        kind: Kind::Str,
                        text: language.to_owned(),
                        line,
                        column,
                    });
                } else if !FICHIERS_LILYPOND.contains(&text.as_str()) {
                    warnings.push(line, column, ignored);
                }
            }
            _ => warnings.push(line, column, ignored),
        }
        // La commande et son argument sont consommés, même ignorés.
        index += 2;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str) -> Vec<String> {
        let mut warnings = Warnings::new();
        expand(source, &mut warnings)
            .iter()
            .map(ToString::to_string)
            .collect()
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
        let tokens = expand(
            "\\include \"articulate.ly\" \\include \"gregorian.ly\" c4",
            &mut warnings,
        );
        assert_eq!(
            tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["c", "4"]
        );
        assert!(warnings.into_vec().is_empty());
    }

    #[test]
    fn fichier_ordinaire_averti() {
        let mut warnings = Warnings::new();
        let tokens = expand("\\include \"macros.ly\" c4", &mut warnings);
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
}
