// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Port sur un sous-ensemble de `scm/define-note-names.scm` (tables de noms de
// notes par langue) et de `lily/pitch.cc` (`Pitch::to_relative_octave`) de
// LilyPond, Copyright (C) 1998--2026 Han-Wen Nienhuys et al., modifié
// (réécrit en Rust) en 2026.
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

//! Hauteurs LilyPond : langues, octaves et transposition.
//!
//! Les tables de noms reprennent `language-pitch-names` de
//! `scm/define-note-names.scm` : toutes les langues livrées par LilyPond
//! (`nederlands`, `catalan`, `deutsch`, `english`, `espanol`, `francais`,
//! `italiano`, `norsk`, `portugues`, `suomi`, `svenska`, `vlaams`) et leurs
//! alias, altérations simples et doubles seulement : les quarts de ton ne sont
//! pas modélisés. Le calcul d'octave relative suit
//! `Pitch::to_relative_octave` (`lily/pitch.cc`) : la hauteur non marquée est
//! placée à l'octave la plus proche de la précédente.

use scoretk_core::model::KeyMode;

use super::ast::PitchSpec;

/// Langue des noms de notes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Language {
    /// Néerlandais, langue par défaut de LilyPond.
    #[default]
    Nederlands,
    /// Catalan.
    Catalan,
    /// Allemand.
    Deutsch,
    /// Anglais.
    English,
    /// Espagnol.
    Espanol,
    /// Français.
    Francais,
    /// Italien.
    Italiano,
    /// Norvégien.
    Norsk,
    /// Portugais.
    Portugues,
    /// Finnois.
    Suomi,
    /// Suédois.
    Svenska,
    /// Flamand.
    Vlaams,
}

impl Language {
    /// Langue correspondant au nom donné à `\language` ou au fichier de langue
    /// de `\include`.
    ///
    /// Le nom est normalisé (minuscules, diacritiques ramenés à l'ASCII, y
    /// compris les formes décomposées comme « c » suivi de U+0327). Les alias
    /// de `define-note-names.scm` sont acceptés : `catalan` pour `català`,
    /// `espanol` pour `español`, `semi-german` pour `deutsch` et `portugues`
    /// pour `português`.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match normalize_name(name).as_str() {
            "nederlands" => Some(Self::Nederlands),
            "catala" | "catalan" => Some(Self::Catalan),
            "deutsch" | "semi-german" => Some(Self::Deutsch),
            "english" => Some(Self::English),
            "espanol" => Some(Self::Espanol),
            "francais" => Some(Self::Francais),
            "italiano" => Some(Self::Italiano),
            "norsk" => Some(Self::Norsk),
            "portugues" => Some(Self::Portugues),
            "suomi" => Some(Self::Suomi),
            "svenska" => Some(Self::Svenska),
            "vlaams" => Some(Self::Vlaams),
            _ => None,
        }
    }

    /// Table des noms : `(nom, degré, altération)`.
    fn names(self) -> &'static [(&'static str, u8, i8)] {
        match self {
            Self::Nederlands => NEDERLANDS,
            Self::Catalan => CATALAN,
            Self::Deutsch => DEUTSCH,
            Self::English => ENGLISH,
            Self::Espanol => ESPANOL,
            Self::Francais => FRANCAIS,
            Self::Italiano => ITALIANO,
            Self::Norsk => NORSK,
            Self::Portugues => PORTUGUES,
            Self::Suomi => SUOMI,
            Self::Svenska => SVENSKA,
            Self::Vlaams => VLAAMS,
        }
    }

    /// Décode un nom de note, en préférant le plus long.
    pub(crate) fn parse(self, word: &str) -> Option<(u8, i8)> {
        let lower = word.to_lowercase();
        self.names()
            .iter()
            .filter(|(name, _, _)| *name == lower)
            .max_by_key(|(name, _, _)| name.len())
            .map(|(_, step, alter)| (*step, *alter))
    }
}

/// Normalise un nom de langue en minuscules ASCII.
///
/// Ramène les lettres accentuées des noms de `language-pitch-names` (`ç`, `é`,
/// `ê`, `à`, `ñ`…) à leur lettre de base et supprime les diacritiques
/// combinants : « français » écrit avec « c » + U+0327 vaut `francais`.
fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter_map(|character| {
            // Diacritiques combinants (U+0300–U+036F) : cédille décomposée…
            if ('\u{300}'..='\u{36f}').contains(&character) {
                return None;
            }
            Some(match character {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
                'ç' => 'c',
                'è' | 'é' | 'ê' | 'ë' => 'e',
                'ì' | 'í' | 'î' | 'ï' => 'i',
                'ñ' => 'n',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
                'ù' | 'ú' | 'û' | 'ü' => 'u',
                'ý' | 'ÿ' => 'y',
                other => other,
            })
        })
        .collect()
}

/// Noms néerlandais (`c`, `cis`, `des`, `ees`, `es`…).
///
/// Les noms à quarts de ton (`cih`, `ceh`, `cisih`, `ceseh`…) sont arrondis
/// au demi-ton MIDI pair le plus proche, comme `ly:pitch->midi` de LilyPond
/// 2.27 (`scm/define-note-names.scm` donne les hauteurs exactes).
const NEDERLANDS: &[(&str, u8, i8)] = &[
    ("ceses", 0, -2),
    ("ceseh", 0, -2),
    ("ceh", 0, 0),
    ("cih", 0, 0),
    ("cisih", 0, 2),
    ("ces", 0, -1),
    ("c", 0, 0),
    ("cis", 0, 1),
    ("cisis", 0, 2),
    ("deseh", 1, -2),
    ("deh", 1, 0),
    ("dih", 1, 0),
    ("disih", 1, 2),
    ("deses", 1, -2),
    ("des", 1, -1),
    ("d", 1, 0),
    ("dis", 1, 1),
    ("disis", 1, 2),
    ("eeseh", 2, -2),
    ("eeh", 2, 0),
    ("eih", 2, 0),
    ("eisih", 2, 2),
    ("eeses", 2, -2),
    ("eses", 2, -2),
    ("ees", 2, -1),
    ("es", 2, -1),
    ("e", 2, 0),
    ("eis", 2, 1),
    ("eisis", 2, 2),
    ("feseh", 3, -1),
    ("feh", 3, -1),
    ("fih", 3, 1),
    ("fisih", 3, 1),
    ("feses", 3, -2),
    ("fes", 3, -1),
    ("f", 3, 0),
    ("fis", 3, 1),
    ("fisis", 3, 2),
    ("geseh", 4, -1),
    ("geh", 4, -1),
    ("gih", 4, 1),
    ("gisih", 4, 1),
    ("geses", 4, -2),
    ("ges", 4, -1),
    ("g", 4, 0),
    ("gis", 4, 1),
    ("gisis", 4, 2),
    ("aeseh", 5, -1),
    ("aeh", 5, -1),
    ("aih", 5, 1),
    ("aisih", 5, 1),
    ("aeses", 5, -2),
    ("ases", 5, -2),
    ("aes", 5, -1),
    ("as", 5, -1),
    ("a", 5, 0),
    ("ais", 5, 1),
    ("aisis", 5, 2),
    ("beseh", 6, -1),
    ("beh", 6, -1),
    ("bih", 6, 1),
    ("bisih", 6, 1),
    ("beses", 6, -2),
    ("bes", 6, -1),
    ("b", 6, 0),
    ("bis", 6, 1),
    ("bisis", 6, 2),
];

/// Noms anglais (`c`, `cs`, `cf`, `css`, `cff`…).
const ENGLISH: &[(&str, u8, i8)] = &[
    ("cff", 0, -2),
    ("cf", 0, -1),
    ("c", 0, 0),
    ("cs", 0, 1),
    ("css", 0, 2),
    ("cx", 0, 2),
    ("dff", 1, -2),
    ("df", 1, -1),
    ("d", 1, 0),
    ("ds", 1, 1),
    ("dss", 1, 2),
    ("dx", 1, 2),
    ("eff", 2, -2),
    ("ef", 2, -1),
    ("e", 2, 0),
    ("es", 2, 1),
    ("ess", 2, 2),
    ("ex", 2, 2),
    ("fff", 3, -2),
    ("ff", 3, -1),
    ("f", 3, 0),
    ("fs", 3, 1),
    ("fss", 3, 2),
    ("fx", 3, 2),
    ("gff", 4, -2),
    ("gf", 4, -1),
    ("g", 4, 0),
    ("gs", 4, 1),
    ("gss", 4, 2),
    ("gx", 4, 2),
    ("aff", 5, -2),
    ("af", 5, -1),
    ("a", 5, 0),
    ("as", 5, 1),
    ("ass", 5, 2),
    ("ax", 5, 2),
    ("bff", 6, -2),
    ("bf", 6, -1),
    ("b", 6, 0),
    ("bs", 6, 1),
    ("bss", 6, 2),
    ("bx", 6, 2),
];

/// Noms allemands (`c`, `cis`, `des`, `es`, `as`, `h`, `b`…).
const DEUTSCH: &[(&str, u8, i8)] = &[
    ("ceses", 0, -2),
    ("ces", 0, -1),
    ("c", 0, 0),
    ("cis", 0, 1),
    ("cisis", 0, 2),
    ("deses", 1, -2),
    ("des", 1, -1),
    ("d", 1, 0),
    ("dis", 1, 1),
    ("disis", 1, 2),
    ("eses", 2, -2),
    ("es", 2, -1),
    ("e", 2, 0),
    ("eis", 2, 1),
    ("eisis", 2, 2),
    ("feses", 3, -2),
    ("fes", 3, -1),
    ("f", 3, 0),
    ("fis", 3, 1),
    ("fisis", 3, 2),
    ("geses", 4, -2),
    ("ges", 4, -1),
    ("g", 4, 0),
    ("gis", 4, 1),
    ("gisis", 4, 2),
    ("asas", 5, -2),
    ("ases", 5, -2),
    ("as", 5, -1),
    ("a", 5, 0),
    ("ais", 5, 1),
    ("aisis", 5, 2),
    ("heses", 6, -2),
    ("b", 6, -1),
    ("h", 6, 0),
    ("his", 6, 1),
    ("hisis", 6, 2),
];

/// Noms italiens (`do`, `dod`, `dob`, `re`…).
const ITALIANO: &[(&str, u8, i8)] = &[
    ("dobb", 0, -2),
    ("dob", 0, -1),
    ("do", 0, 0),
    ("dod", 0, 1),
    ("dodd", 0, 2),
    ("rebb", 1, -2),
    ("reb", 1, -1),
    ("re", 1, 0),
    ("red", 1, 1),
    ("redd", 1, 2),
    ("mibb", 2, -2),
    ("mib", 2, -1),
    ("mi", 2, 0),
    ("mid", 2, 1),
    ("midd", 2, 2),
    ("fabb", 3, -2),
    ("fab", 3, -1),
    ("fa", 3, 0),
    ("fad", 3, 1),
    ("fadd", 3, 2),
    ("solbb", 4, -2),
    ("solb", 4, -1),
    ("sol", 4, 0),
    ("sold", 4, 1),
    ("soldd", 4, 2),
    ("labb", 5, -2),
    ("lab", 5, -1),
    ("la", 5, 0),
    ("lad", 5, 1),
    ("ladd", 5, 2),
    ("sibb", 6, -2),
    ("sib", 6, -1),
    ("si", 6, 0),
    ("sid", 6, 1),
    ("sidd", 6, 2),
];

/// Noms français (`do`, `ré`, `mib`, `sold`…), avec les variantes sans accent.
const FRANCAIS: &[(&str, u8, i8)] = &[
    ("dobb", 0, -2),
    ("dob", 0, -1),
    ("do", 0, 0),
    ("dod", 0, 1),
    ("dodd", 0, 2),
    ("rébb", 1, -2),
    ("rebb", 1, -2),
    ("réb", 1, -1),
    ("reb", 1, -1),
    ("ré", 1, 0),
    ("re", 1, 0),
    ("réd", 1, 1),
    ("red", 1, 1),
    ("rédd", 1, 2),
    ("redd", 1, 2),
    ("mibb", 2, -2),
    ("mib", 2, -1),
    ("mi", 2, 0),
    ("mid", 2, 1),
    ("midd", 2, 2),
    ("fabb", 3, -2),
    ("fab", 3, -1),
    ("fa", 3, 0),
    ("fad", 3, 1),
    ("fadd", 3, 2),
    ("solbb", 4, -2),
    ("solb", 4, -1),
    ("sol", 4, 0),
    ("sold", 4, 1),
    ("soldd", 4, 2),
    ("labb", 5, -2),
    ("lab", 5, -1),
    ("la", 5, 0),
    ("lad", 5, 1),
    ("ladd", 5, 2),
    ("sibb", 6, -2),
    ("sib", 6, -1),
    ("si", 6, 0),
    ("sid", 6, 1),
    ("sidd", 6, 2),
];

/// Noms catalans (`do`, `dod`/`dos`, `reb`…), avec les variantes en `s`.
const CATALAN: &[(&str, u8, i8)] = &[
    ("dobb", 0, -2),
    ("dob", 0, -1),
    ("do", 0, 0),
    ("dod", 0, 1),
    ("dos", 0, 1),
    ("dodd", 0, 2),
    ("doss", 0, 2),
    ("rebb", 1, -2),
    ("reb", 1, -1),
    ("re", 1, 0),
    ("red", 1, 1),
    ("res", 1, 1),
    ("redd", 1, 2),
    ("ress", 1, 2),
    ("mibb", 2, -2),
    ("mib", 2, -1),
    ("mi", 2, 0),
    ("mid", 2, 1),
    ("mis", 2, 1),
    ("midd", 2, 2),
    ("miss", 2, 2),
    ("fabb", 3, -2),
    ("fab", 3, -1),
    ("fa", 3, 0),
    ("fad", 3, 1),
    ("fas", 3, 1),
    ("fadd", 3, 2),
    ("fass", 3, 2),
    ("solbb", 4, -2),
    ("solb", 4, -1),
    ("sol", 4, 0),
    ("sold", 4, 1),
    ("sols", 4, 1),
    ("soldd", 4, 2),
    ("solss", 4, 2),
    ("labb", 5, -2),
    ("lab", 5, -1),
    ("la", 5, 0),
    ("lad", 5, 1),
    ("las", 5, 1),
    ("ladd", 5, 2),
    ("lass", 5, 2),
    ("sibb", 6, -2),
    ("sib", 6, -1),
    ("si", 6, 0),
    ("sid", 6, 1),
    ("sis", 6, 1),
    ("sidd", 6, 2),
    ("siss", 6, 2),
];

/// Noms espagnols (`do`, `dos`, `reb`…), avec les double-dièses en `x`.
const ESPANOL: &[(&str, u8, i8)] = &[
    ("dobb", 0, -2),
    ("dob", 0, -1),
    ("do", 0, 0),
    ("dos", 0, 1),
    ("doss", 0, 2),
    ("dox", 0, 2),
    ("rebb", 1, -2),
    ("reb", 1, -1),
    ("re", 1, 0),
    ("res", 1, 1),
    ("ress", 1, 2),
    ("rex", 1, 2),
    ("mibb", 2, -2),
    ("mib", 2, -1),
    ("mi", 2, 0),
    ("mis", 2, 1),
    ("miss", 2, 2),
    ("mix", 2, 2),
    ("fabb", 3, -2),
    ("fab", 3, -1),
    ("fa", 3, 0),
    ("fas", 3, 1),
    ("fass", 3, 2),
    ("fax", 3, 2),
    ("solbb", 4, -2),
    ("solb", 4, -1),
    ("sol", 4, 0),
    ("sols", 4, 1),
    ("solss", 4, 2),
    ("solx", 4, 2),
    ("labb", 5, -2),
    ("lab", 5, -1),
    ("la", 5, 0),
    ("las", 5, 1),
    ("lass", 5, 2),
    ("lax", 5, 2),
    ("sibb", 6, -2),
    ("sib", 6, -1),
    ("si", 6, 0),
    ("sis", 6, 1),
    ("siss", 6, 2),
    ("six", 6, 2),
];

/// Noms norvégiens (`c`, `cess`/`cis`, `b`, `h`…).
const NORSK: &[(&str, u8, i8)] = &[
    ("cessess", 0, -2),
    ("ceses", 0, -2),
    ("cess", 0, -1),
    ("ces", 0, -1),
    ("c", 0, 0),
    ("ciss", 0, 1),
    ("cis", 0, 1),
    ("cississ", 0, 2),
    ("cisis", 0, 2),
    ("dessess", 1, -2),
    ("deses", 1, -2),
    ("dess", 1, -1),
    ("des", 1, -1),
    ("d", 1, 0),
    ("diss", 1, 1),
    ("dis", 1, 1),
    ("dississ", 1, 2),
    ("disis", 1, 2),
    ("eessess", 2, -2),
    ("eeses", 2, -2),
    ("essess", 2, -2),
    ("eses", 2, -2),
    ("eess", 2, -1),
    ("ees", 2, -1),
    ("ess", 2, -1),
    ("es", 2, -1),
    ("e", 2, 0),
    ("eiss", 2, 1),
    ("eis", 2, 1),
    ("eississ", 2, 2),
    ("eisis", 2, 2),
    ("fessess", 3, -2),
    ("feses", 3, -2),
    ("fess", 3, -1),
    ("fes", 3, -1),
    ("f", 3, 0),
    ("fiss", 3, 1),
    ("fis", 3, 1),
    ("fississ", 3, 2),
    ("fisis", 3, 2),
    ("gessess", 4, -2),
    ("geses", 4, -2),
    ("gess", 4, -1),
    ("ges", 4, -1),
    ("g", 4, 0),
    ("giss", 4, 1),
    ("gis", 4, 1),
    ("gississ", 4, 2),
    ("gisis", 4, 2),
    ("aessess", 5, -2),
    ("assess", 5, -2),
    ("aeses", 5, -2),
    ("ases", 5, -2),
    ("aess", 5, -1),
    ("ass", 5, -1),
    ("as", 5, -1),
    ("aes", 5, -1),
    ("a", 5, 0),
    ("aiss", 5, 1),
    ("ais", 5, 1),
    ("aississ", 5, 2),
    ("aisis", 5, 2),
    // `bes` vaut le double-bémol dans cette table amont, comme `bess`.
    ("bess", 6, -2),
    ("bes", 6, -2),
    ("b", 6, -1),
    ("h", 6, 0),
    ("hiss", 6, 1),
    ("his", 6, 1),
    ("hississ", 6, 2),
    ("hisis", 6, 2),
];

/// Noms portugais (`do`, `dos`, `reb`…).
const PORTUGUES: &[(&str, u8, i8)] = &[
    ("dobb", 0, -2),
    ("dob", 0, -1),
    ("do", 0, 0),
    ("dos", 0, 1),
    ("doss", 0, 2),
    ("rebb", 1, -2),
    ("reb", 1, -1),
    ("re", 1, 0),
    ("res", 1, 1),
    ("ress", 1, 2),
    ("mibb", 2, -2),
    ("mib", 2, -1),
    ("mi", 2, 0),
    ("mis", 2, 1),
    ("miss", 2, 2),
    ("fabb", 3, -2),
    ("fab", 3, -1),
    ("fa", 3, 0),
    ("fas", 3, 1),
    ("fass", 3, 2),
    ("solbb", 4, -2),
    ("solb", 4, -1),
    ("sol", 4, 0),
    ("sols", 4, 1),
    ("solss", 4, 2),
    ("labb", 5, -2),
    ("lab", 5, -1),
    ("la", 5, 0),
    ("las", 5, 1),
    ("lass", 5, 2),
    ("sibb", 6, -2),
    ("sib", 6, -1),
    ("si", 6, 0),
    ("sis", 6, 1),
    ("siss", 6, 2),
];

/// Noms finnois (`c`, `cis`, `b`, `h`…).
const SUOMI: &[(&str, u8, i8)] = &[
    ("ceses", 0, -2),
    ("ces", 0, -1),
    ("c", 0, 0),
    ("cis", 0, 1),
    ("cisis", 0, 2),
    ("deses", 1, -2),
    ("des", 1, -1),
    ("d", 1, 0),
    ("dis", 1, 1),
    ("disis", 1, 2),
    ("eses", 2, -2),
    ("es", 2, -1),
    ("e", 2, 0),
    ("eis", 2, 1),
    ("eisis", 2, 2),
    ("feses", 3, -2),
    ("fes", 3, -1),
    ("f", 3, 0),
    ("fis", 3, 1),
    ("fisis", 3, 2),
    ("geses", 4, -2),
    ("ges", 4, -1),
    ("g", 4, 0),
    ("gis", 4, 1),
    ("gisis", 4, 2),
    ("asas", 5, -2),
    ("ases", 5, -2),
    ("as", 5, -1),
    ("a", 5, 0),
    ("ais", 5, 1),
    ("aisis", 5, 2),
    ("bes", 6, -2),
    ("heses", 6, -2),
    // Entrée invalide conservée par l'amont pour la rétrocompatibilité.
    ("bb", 6, -2),
    ("b", 6, -1),
    ("h", 6, 0),
    ("his", 6, 1),
    ("hisis", 6, 2),
];

/// Noms suédois (`c`, `ciss`, `b`, `h`…).
const SVENSKA: &[(&str, u8, i8)] = &[
    ("cessess", 0, -2),
    ("cess", 0, -1),
    ("c", 0, 0),
    ("ciss", 0, 1),
    ("cississ", 0, 2),
    ("dessess", 1, -2),
    ("dess", 1, -1),
    ("d", 1, 0),
    ("diss", 1, 1),
    ("dississ", 1, 2),
    ("essess", 2, -2),
    ("ess", 2, -1),
    ("e", 2, 0),
    ("eiss", 2, 1),
    ("eississ", 2, 2),
    ("fessess", 3, -2),
    ("fess", 3, -1),
    ("f", 3, 0),
    ("fiss", 3, 1),
    ("fississ", 3, 2),
    ("gessess", 4, -2),
    ("gess", 4, -1),
    ("g", 4, 0),
    ("giss", 4, 1),
    ("gississ", 4, 2),
    ("assess", 5, -2),
    ("ass", 5, -1),
    ("a", 5, 0),
    ("aiss", 5, 1),
    ("aississ", 5, 2),
    ("hessess", 6, -2),
    ("b", 6, -1),
    ("h", 6, 0),
    ("hiss", 6, 1),
    ("hississ", 6, 2),
];

/// Noms flamands (`do`, `dok`, `reb`…), dièse en `k`.
const VLAAMS: &[(&str, u8, i8)] = &[
    ("dobb", 0, -2),
    ("dob", 0, -1),
    ("do", 0, 0),
    ("dok", 0, 1),
    ("dokk", 0, 2),
    ("rebb", 1, -2),
    ("reb", 1, -1),
    ("re", 1, 0),
    ("rek", 1, 1),
    ("rekk", 1, 2),
    ("mibb", 2, -2),
    ("mib", 2, -1),
    ("mi", 2, 0),
    ("mik", 2, 1),
    ("mikk", 2, 2),
    ("fabb", 3, -2),
    ("fab", 3, -1),
    ("fa", 3, 0),
    ("fak", 3, 1),
    ("fakk", 3, 2),
    ("solbb", 4, -2),
    ("solb", 4, -1),
    ("sol", 4, 0),
    ("solk", 4, 1),
    ("solkk", 4, 2),
    ("labb", 5, -2),
    ("lab", 5, -1),
    ("la", 5, 0),
    ("lak", 5, 1),
    ("lakk", 5, 2),
    ("sibb", 6, -2),
    ("sib", 6, -1),
    ("si", 6, 0),
    ("sik", 6, 1),
    ("sikk", 6, 2),
];

/// Demi-tons d'un degré diatonique.
const SEMITONES: [i8; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Décalage de quinte d'un degré naturel (do, ré, mi, fa, sol, la, si).
const FIFTH_OFFSETS: [i8; 7] = [0, 2, 4, -1, 1, 3, 5];

/// Classe de hauteur tonale (`tpc`) d'un degré et d'une altération.
///
/// La spirale des quintes est partagée avec l'écriture
/// (`scoretk_lilypond_common::pitch`) : `tpc` vaut 13 pour fa naturel, puis
/// avance d'une quinte par degré.
pub(crate) use scoretk_lilypond_common::pitch::tpc_of;

/// Hauteur MIDI d'une hauteur écrite, l'octave 4 contenant le do central.
pub(crate) fn midi_of(step: u8, alter: i8, octave: i8) -> Option<u8> {
    let value =
        i32::from(octave) * 12 + i32::from(SEMITONES[usize::from(step)]) + i32::from(alter) + 12;
    u8::try_from(value).ok().filter(|pitch| *pitch <= 127)
}

/// Nombre de quintes d'une tonique (`\key`).
pub(crate) fn fifths_of(step: u8, alter: i8) -> i8 {
    FIFTH_OFFSETS[usize::from(step)] + 7 * alter
}

/// Décalage en quintes du mode par rapport au majeur
/// (`ly/scale-definitions-init.ly`) : `minor`/`aeolian` a 3 bémols de plus,
/// `dorian` 2, `phrygian` 4, `locrian` 5 ; `lydian` a 1 dièse de plus et
/// `mixolydian` 1 bémol de plus. Le nom sans mode (déjà majeur) garde 0.
pub(crate) fn mode_fifths_offset(word: &str) -> i8 {
    match word {
        "minor" | "aeolian" => -3,
        "dorian" => -2,
        "phrygian" => -4,
        "locrian" => -5,
        "lydian" => 1,
        "mixolydian" => -1,
        _ => 0,
    }
}

/// Intervalle de `\transposition` par rapport au do central écrit.
///
/// Le réglage amont vaut `c'` : `\transposition c` fait donc sonner une octave
/// plus bas, `\transposition a'` une sixte majeure plus haut. Les deux nombres
/// renvoyés sont l'intervalle diatonique (degrés) et chromatique (demi-tons),
/// tels que `scoretk_core::model::Transpose` les attend.
pub(crate) fn instrument_transpose(pitch: &PitchSpec) -> (i32, i32) {
    let diatonic = i32::from(pitch.octave) * 7 + i32::from(pitch.step) - 7;
    let chromatic = i32::from(pitch.octave) * 12
        + i32::from(SEMITONES[usize::from(pitch.step.min(6))])
        + i32::from(pitch.alter)
        - 12;
    (diatonic, chromatic)
}

/// Mode d'armure d'un mot de `\key`.
pub(crate) fn key_mode(word: &str) -> KeyMode {
    match word {
        // MuseScore ne stocke un mode que s'il est explicite dans le fichier
        // `.mscx` ; l'écrivain écrit `\major` aussi bien pour un majeur
        // explicite que pour une armure sans mode. Pour que l'aller-retour
        // retrouve l'absence de `<mode>`, `\major` est lu comme inconnu ;
        // `\minor` reste mineur, seul mode que MuseScore distingue vraiment.
        "minor" | "aeolian" => KeyMode::Minor,
        _ => KeyMode::Unknown,
    }
}

/// Résolution d'octave d'une hauteur écrite.
///
/// `reference` est l'octave de référence en mode `\fixed`/`\relative` (celle
/// de `\fixed c'`, soit 4 par défaut) ; `previous` est la dernière hauteur
/// résolue en mode relatif, avec son degré et son octave.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OctaveState {
    /// Octave de référence (0 = octave de `c,`, 4 = do central).
    pub reference: i8,
    /// Dernière hauteur résolue (degré, octave).
    pub previous: Option<(u8, i8)>,
    /// Mode relatif.
    pub relative: bool,
}

impl Default for OctaveState {
    fn default() -> Self {
        Self {
            // En notation absolue, un nom sans marque vaut `c` (do deux
            // octaves sous le do central, octave 3 dans notre numérotation).
            reference: 3,
            previous: None,
            relative: false,
        }
    }
}

impl OctaveState {
    /// Octave absolue d'une hauteur écrite.
    ///
    /// En mode `\fixed` (ou absolu), l'octave est celle de la référence plus
    /// les marques ; en mode `\relative`, la hauteur non marquée est placée à
    /// l'octave la plus proche de la précédente, comme
    /// `Pitch::to_relative_octave` dans `lily/pitch.cc`.
    pub(crate) fn resolve(&mut self, pitch: &PitchSpec) -> i8 {
        let octave = if let Some(check) = pitch.octave_check {
            // `n='` : la hauteur vérifiée est absolue, indépendante de la
            // note précédente et de la référence `\relative`
            // (`ly/relative-octave-check.cc`).
            3i8.saturating_add(check)
        } else if self.relative {
            match self.previous {
                Some((previous_step, previous_octave)) => {
                    relative_octave(previous_step, previous_octave, pitch.step)
                        .saturating_add(pitch.octave)
                }
                None => self.reference.saturating_add(pitch.octave),
            }
        } else {
            self.reference.saturating_add(pitch.octave)
        };
        // La borne atteinte reste visible : `midi_of` écarte la hauteur hors de
        // 0..127 et l'événement avertit, comme pour les accords relatifs.
        self.previous = Some((pitch.step, octave));
        octave
    }

    /// `\octaveCheck` : place la hauteur de contrôle relative à la dernière
    /// hauteur, et décale la référence de l'écart d'octave en cas d'échec
    /// (`lily/relative-octave-check.cc:33-60`). Rend le message d'échec.
    pub(crate) fn check_octave(&mut self, pitch: &PitchSpec) -> Option<String> {
        let attendu = i16::from(pitch.octave) + 3;
        let resultat = match self.previous {
            Some((step, octave)) => i16::from(relative_octave(step, octave, pitch.step)),
            None => i16::from(self.reference),
        };
        if resultat == attendu {
            return None;
        }
        let delta = attendu - resultat;
        let message = format!(
            "Failed octave check, got: octave {} instead of {}",
            resultat, attendu
        );
        match self.previous {
            Some((step, octave)) => {
                self.previous = Some((step, borne_octave(i16::from(octave) + delta)));
            }
            None => self.reference = borne_octave(i16::from(self.reference) + delta),
        }
        Some(message)
    }

    /// Définit la référence à partir de la hauteur écrite en argument
    /// (`\fixed c'`, `\relative c''`).
    ///
    /// L'argument est lu en notation absolue : sa valeur est
    /// `reference - 1 + marques`, la référence valant 3 (`c`, deux octaves
    /// sous le do central) hors mode relatif.
    pub(crate) fn set_reference(&mut self, pitch: &PitchSpec) {
        self.reference = 3i8.saturating_add(pitch.octave);
        self.previous = None;
    }

    /// Ouvre un bloc `\relative`.
    ///
    /// La première note se place, comme les suivantes, à l'octave la plus
    /// proche de la référence, degré compris (`\relative f'` ≠ `\relative c'`).
    /// La référence joue donc le rôle de note précédente, comme la hauteur
    /// initiale passée à `Music::make_relative` (`lily/music.cc`).
    ///
    /// Sans hauteur, LilyPond prend `f` sous le do central : la fonction
    /// `relative` de `ly/music-functions-init.ly` construit
    /// `(ly:make-pitch -1 (quotient (ly:pitch-steps (ly:make-pitch 1 0)) 2))`,
    /// soit fa en octave -1 (`f` pour une gamme de sept degrés), donc l'octave
    /// 3 dans notre numérotation.
    pub(crate) fn start_relative(&mut self, reference: Option<&PitchSpec>) {
        self.relative = true;
        match reference {
            Some(pitch) => {
                self.set_reference(pitch);
                self.previous = Some((pitch.step, self.reference));
            }
            None => {
                self.reference = 3;
                self.previous = Some((3, 3));
            }
        }
    }

    /// État d'octave d'une musique que la conversion `\relative` ne parcourt
    /// pas (`\transpose`, `\partCombine`, `\absolute`…).
    ///
    /// Sous `\relative`, on repart de la référence absolue par défaut ; sous
    /// `\fixed`, la référence courante est conservée car la transposition
    /// s'applique bien aux parties. C'est le `to-relative-callback`
    /// `ly:relative-octave-music::no-relative-callback` porté par
    /// `TransposedMusic` et `UnrelativableMusic`
    /// (`scm/define-music-types.scm`) : `Music::to_relative_octave` renvoie
    /// alors la hauteur reçue sans descendre dans le bloc. Notation Reference
    /// §1.2.2 : « Music inside a `\transpose` block is absolute unless a
    /// `\relative` is included in the block. »
    pub(crate) fn sans_relative(self) -> Self {
        if self.relative {
            Self::default()
        } else {
            Self {
                relative: false,
                ..self
            }
        }
    }
}

/// Ramène une octave calculée en `i16` dans la plage `i8` en saturant.
///
/// Un `unwrap_or(0)` ramènerait une octave débordée à zéro et la musique
/// repartirait du do central en silence : la borne est conservée pour que la
/// note soit ensuite écartée avec un avertissement.
fn borne_octave(octave: i16) -> i8 {
    octave.clamp(i16::from(i8::MIN), i16::from(i8::MAX)) as i8
}

/// Octave d'une hauteur relative à la précédente.
///
/// Reprend l'algorithme de `Pitch::to_relative_octave`
/// (`lily/pitch.cc`) : la hauteur monte ou descend au degré demandé, et
/// l'octave choisie est celle dont l'écart diatonique est le plus petit.
fn relative_octave(previous_step: u8, previous_octave: i8, step: u8) -> i8 {
    let previous_step = i16::from(previous_step);
    let step = i16::from(step);
    let previous_octave = i16::from(previous_octave);
    let up_octave = if previous_step > step {
        previous_octave + 1
    } else {
        previous_octave
    };
    let down_octave = if previous_step < step {
        previous_octave - 1
    } else {
        previous_octave
    };
    let reference = previous_step + previous_octave * 7;
    let up_distance = (step + up_octave * 7 - reference).abs();
    let down_distance = (step + down_octave * 7 - reference).abs();
    if up_distance < down_distance {
        borne_octave(up_octave)
    } else {
        borne_octave(down_octave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::tpc;

    fn spec(step: u8, octave: i8) -> PitchSpec {
        PitchSpec {
            step,
            alter: 0,
            octave,
            octave_check: None,
            explicit_octave: octave != 0,
            force: false,
            cautionary: false,
        }
    }

    #[test]
    fn noms_par_langue() {
        assert_eq!(Language::Nederlands.parse("cis"), Some((0, 1)));
        assert_eq!(Language::Nederlands.parse("es"), Some((2, -1)));
        assert_eq!(Language::English.parse("bf"), Some((6, -1)));
        assert_eq!(Language::English.parse("css"), Some((0, 2)));
        assert_eq!(Language::Deutsch.parse("h"), Some((6, 0)));
        assert_eq!(Language::Deutsch.parse("b"), Some((6, -1)));
        assert_eq!(Language::Italiano.parse("sold"), Some((4, 1)));
        assert_eq!(Language::Francais.parse("réb"), Some((1, -1)));
        assert_eq!(Language::Francais.parse("re"), Some((1, 0)));
        assert_eq!(Language::Nederlands.parse("z"), None);
    }

    #[test]
    fn langues_complementaires() {
        assert_eq!(Language::Catalan.parse("dod"), Some((0, 1)));
        assert_eq!(Language::Catalan.parse("dos"), Some((0, 1)));
        assert_eq!(Language::Catalan.parse("sib"), Some((6, -1)));
        assert_eq!(Language::Espanol.parse("do"), Some((0, 0)));
        assert_eq!(Language::Espanol.parse("sib"), Some((6, -1)));
        assert_eq!(Language::Espanol.parse("dox"), Some((0, 2)));
        assert_eq!(Language::Norsk.parse("cess"), Some((0, -1)));
        assert_eq!(Language::Norsk.parse("h"), Some((6, 0)));
        assert_eq!(Language::Norsk.parse("b"), Some((6, -1)));
        assert_eq!(Language::Portugues.parse("dos"), Some((0, 1)));
        assert_eq!(Language::Portugues.parse("sibb"), Some((6, -2)));
        assert_eq!(Language::Suomi.parse("h"), Some((6, 0)));
        assert_eq!(Language::Suomi.parse("as"), Some((5, -1)));
        assert_eq!(Language::Svenska.parse("ciss"), Some((0, 1)));
        assert_eq!(Language::Svenska.parse("b"), Some((6, -1)));
        assert_eq!(Language::Vlaams.parse("dok"), Some((0, 1)));
        assert_eq!(Language::Vlaams.parse("sikk"), Some((6, 2)));
    }

    #[test]
    fn noms_de_langue_normalises() {
        assert_eq!(Language::from_name("italiano"), Some(Language::Italiano));
        assert_eq!(Language::from_name("català"), Some(Language::Catalan));
        assert_eq!(Language::from_name("catalan"), Some(Language::Catalan));
        assert_eq!(Language::from_name("español"), Some(Language::Espanol));
        assert_eq!(Language::from_name("espanol"), Some(Language::Espanol));
        assert_eq!(Language::from_name("français"), Some(Language::Francais));
        assert_eq!(Language::from_name("português"), Some(Language::Portugues));
        // « c » suivi de U+0327 (cédille combinante).
        assert_eq!(
            Language::from_name("franc\u{327}ais"),
            Some(Language::Francais)
        );
        assert_eq!(Language::from_name("Francais"), Some(Language::Francais));
        assert_eq!(Language::from_name("klingon"), None);
    }

    #[test]
    fn hauteurs() {
        assert_eq!(midi_of(0, 0, 4), Some(60));
        assert_eq!(midi_of(6, -1, 3), Some(58));
        assert_eq!(tpc_of(0, 0), tpc::C);
        assert_eq!(tpc_of(0, 1), tpc::C_SHARP);
        assert_eq!(tpc_of(6, -1), tpc::B_FLAT);
        assert_eq!(fifths_of(4, 0), 1);
        assert_eq!(fifths_of(1, -1), -5);
        assert_eq!(fifths_of(3, 1), 6);
    }

    #[test]
    fn octave_fixe() {
        let mut state = OctaveState::default();
        assert_eq!(state.resolve(&spec(0, 0)), 3);
        assert_eq!(state.resolve(&spec(0, 1)), 4);
        assert_eq!(state.resolve(&spec(0, -1)), 2);
        state.set_reference(&spec(0, 1));
        assert_eq!(state.reference, 4);
        assert_eq!(state.resolve(&spec(0, 0)), 4);
        assert_eq!(state.resolve(&spec(0, 1)), 5);
    }

    #[test]
    fn octave_relative() {
        let mut state = OctaveState {
            relative: true,
            reference: 4,
            ..OctaveState::default()
        };
        assert_eq!(state.resolve(&spec(0, 0)), 4);
        // Une seconde au-dessus : même octave.
        assert_eq!(state.resolve(&spec(1, 0)), 4);
        // Retour au do : même octave (quarte descendante préférée à la quinte
        // montante).
        assert_eq!(state.resolve(&spec(0, 0)), 4);
        // Un si non marqué descend d'une octave.
        assert_eq!(state.resolve(&spec(6, 0)), 3);
    }

    #[test]
    fn octave_relative_bornee() {
        // Après 200 reprises, l'octave relative approche de la borne i8 : une
        // marque d'octave supplémentaire doit saturer, pas déborder.
        let mut state = OctaveState {
            relative: true,
            reference: 126,
            previous: Some((0, 126)),
        };
        assert_eq!(state.resolve(&spec(0, 1)), 127);
        assert_eq!(state.resolve(&spec(0, 1)), 127);
        assert_eq!(state.resolve(&spec(0, -1)), 126);
        // Symétrie : une descente répétée sature elle aussi.
        let mut state = OctaveState {
            relative: true,
            reference: -126,
            previous: Some((0, -126)),
        };
        assert_eq!(state.resolve(&spec(0, -1)), -127);
        assert_eq!(state.resolve(&spec(0, -1)), -128);
        assert_eq!(state.resolve(&spec(0, -1)), -128);
    }
}
