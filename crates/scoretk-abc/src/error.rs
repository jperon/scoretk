// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// L'écriture et la lecture ABC suivent la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>) et le comportement de
// `xml2abc.py` et `abc2xml.py` de Willem Vree (LGPL, sans version précisée
// dans leur en-tête ; conversion en GPL-3.0-only autorisée par la LGPL 2.1
// § 3). Aucune ligne de ces scripts n'est copiée : la mise en œuvre est une
// réécriture Rust.
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

//! Erreurs et avertissements de `scoretk-abc`.

use std::fmt;

/// Erreur de lecture ou d'écriture ABC.
///
/// L'écriture ne peut échouer que sur une partition inexploitable ; les
/// valeurs douteuses du modèle (durée nulle, piste hors portée) sont
/// remplacées par une valeur de repli. La lecture, elle, signale les lignes
/// qu'elle ne comprend pas : une note manquante fausserait silencieusement la
/// musique.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// La partition ne contient aucune portée : il n'y a aucune voix à écrire.
    #[error("partition sans portée : rien à écrire en ABC")]
    PartitionVide,

    /// La source ne contient aucune en-tête `X:`.
    #[error("aucun morceau ABC : l'en-tête X: est absent")]
    AucunMorceau,

    /// L'indice de morceau demandé dépasse le nombre de morceaux du fichier.
    #[error("morceau {index} demandé, mais le fichier n'en contient que {count}")]
    MorceauHorsLimite {
        /// Indice demandé (base zéro).
        index: usize,
        /// Nombre de morceaux trouvés.
        count: usize,
    },

    /// Un en-tête obligatoire manque (`K:` le plus souvent).
    #[error("en-tête ABC incomplet : {0}")]
    EnTeteIncomplet(&'static str),

    /// Ligne illisible.
    #[error("ligne {line} : {message}")]
    LigneInvalide {
        /// Numéro de ligne dans la source (base un).
        line: usize,
        /// Diagnostic.
        message: String,
    },

    /// Indication de mesure illisible.
    #[error("ligne {line} : indication de mesure « {text} » invalide")]
    MesureInvalide {
        /// Numéro de ligne dans la source (base un).
        line: usize,
        /// Texte fautif.
        text: String,
    },

    /// Tonalité illisible.
    #[error("ligne {line} : tonalité « {text} » invalide")]
    TonaliteInvalide {
        /// Numéro de ligne dans la source (base un).
        line: usize,
        /// Texte fautif.
        text: String,
    },

    /// Durée illisible.
    #[error("ligne {line} : durée « {text} » invalide")]
    DureeInvalide {
        /// Numéro de ligne dans la source (base un).
        line: usize,
        /// Texte fautif.
        text: String,
    },

    /// Note rencontrée avant toute définition de voix.
    #[error("ligne {line} : note sans voix ; ajouter V:1 avant la musique")]
    NoteSansVoix {
        /// Numéro de ligne dans la source (base un).
        line: usize,
    },
}

/// Avertissement de lecture, non bloquant.
///
/// Les constructions ABC que le sous-ensemble ne traduit pas (`%%` inconnus,
/// mode non majeur/mineur, ornement sans équivalent…) produisent un
/// avertissement au lieu d'une erreur : la partition reste lisible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// Numéro de ligne dans la source (base un), zéro pour un avertissement
    /// global.
    pub line: usize,
    /// Diagnostic.
    pub message: String,
}

impl Warning {
    /// Construit un avertissement.
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "avertissement ABC : {}", self.message)
        } else {
            write!(
                f,
                "avertissement ABC ligne {} : {}",
                self.line, self.message
            )
        }
    }
}

/// Résultat d'une opération de `scoretk-abc`.
pub type Result<T> = std::result::Result<T, Error>;
