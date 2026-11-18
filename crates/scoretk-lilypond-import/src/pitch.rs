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
//! Les tables de noms reprennent `scm/define-note-names.scm` pour les cinq
//! langues demandées (`nederlands`, `english`, `deutsch`, `italiano`,
//! `français`), altérations simples et doubles seulement : les quarts de ton
//! ne sont pas modélisés. Le calcul d'octave relative suit
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
    /// Anglais.
    English,
    /// Allemand.
    Deutsch,
    /// Italien.
    Italiano,
    /// Français.
    Francais,
}

impl Language {
    /// Langue correspondant au nom donné à `\language`.
    ///
    /// Les alias de `define-note-names.scm` sont acceptés (`francais` sans
    /// cédille, `espanol` non couvert).
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().replace('ç', "c").as_str() {
            "nederlands" => Some(Self::Nederlands),
            "english" => Some(Self::English),
            "deutsch" | "semi-german" => Some(Self::Deutsch),
            "italiano" => Some(Self::Italiano),
            "francais" => Some(Self::Francais),
            _ => None,
        }
    }

    /// Table des noms : `(nom, degré, altération)`.
    fn names(self) -> &'static [(&'static str, u8, i8)] {
        match self {
            Self::Nederlands => NEDERLANDS,
            Self::English => ENGLISH,
            Self::Deutsch => DEUTSCH,
            Self::Italiano => ITALIANO,
            Self::Francais => FRANCAIS,
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

/// Noms néerlandais (`c`, `cis`, `des`, `ees`, `es`…).
const NEDERLANDS: &[(&str, u8, i8)] = &[
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
    ("eeses", 2, -2),
    ("eses", 2, -2),
    ("ees", 2, -1),
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
    ("aeses", 5, -2),
    ("ases", 5, -2),
    ("aes", 5, -1),
    ("as", 5, -1),
    ("a", 5, 0),
    ("ais", 5, 1),
    ("aisis", 5, 2),
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
        let octave = if self.relative {
            match self.previous {
                Some((previous_step, previous_octave)) => {
                    relative_octave(previous_step, previous_octave, pitch.step) + pitch.octave
                }
                None => self.reference + pitch.octave,
            }
        } else {
            self.reference + pitch.octave
        };
        self.previous = Some((pitch.step, octave));
        octave
    }

    /// Définit la référence à partir de la hauteur écrite en argument
    /// (`\fixed c'`, `\relative c''`).
    ///
    /// L'argument est lu en notation absolue : sa valeur est
    /// `reference - 1 + marques`, la référence valant 3 (`c`, deux octaves
    /// sous le do central) hors mode relatif.
    pub(crate) fn set_reference(&mut self, pitch: &PitchSpec) {
        self.reference = 3 + pitch.octave;
        self.previous = None;
    }
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
        i8::try_from(up_octave).unwrap_or(0)
    } else {
        i8::try_from(down_octave).unwrap_or(0)
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
}
