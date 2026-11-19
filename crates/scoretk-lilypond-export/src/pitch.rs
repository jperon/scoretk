// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les tables de correspondance reprennent le comportement de `musicxml2ly`
// (paquet LilyPond, GPL-3.0-or-later), sans en copier de code : noms de notes
// du mode `\fixed`, noms de clés et armures.
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

//! Hauteurs, armures et clés.
//!
//! Le modèle porte deux informations de hauteur : [`Note::pitch`], la hauteur
//! MIDI **réelle** (celle du fichier `.mscx`), et [`Note::tpc`], la classe de
//! hauteur tonale qui fixe l'orthographe (do dièse plutôt que ré bémol). Le
//! mode `\fixed` de LilyPond demande les deux : le nom orthographié et une
//! octave absolue.
//!
//! # Octave
//!
//! `\fixed c'` prend `c'` (do central, MIDI 60) comme hauteur de référence :
//! une note non marquée vaut l'octave 4, `'` ajoute une octave et `,` en
//! retire une. `pitch_token` produit donc les marques `octave - 4`.
//!
//! # Orthographe
//!
//! `tpc` parcourt la spirale des quintes : `tpc - TPC_MIN` modulo 7 donne le
//! degré (fa, do, sol, ré, la, mi, si) et la division entière par 7 donne
//! l'altération, comme `tpc2stepName` et `tpc2alter` de MuseScore
//! (`engraving/dom/pitchspelling.cpp`). Les altérations s'écrivent en anglais :
//! `f` (bémol), `s` (dièse), répétés pour les altérations doubles et triples.

use scoretk_core::model::{Clef, KeyMode, KeySig, Note, NoteHeadGroup};
use scoretk_lilypond_common::clefs;
use scoretk_lilypond_common::pitch::{alter, step, valid};

/// Suffixe d'altération en notation anglaise (`f`, `s`, `ff`, `ss`…).
fn alter_suffix(alter: i32) -> String {
    let (letter, count) = if alter < 0 {
        ('f', -alter)
    } else {
        ('s', alter)
    };
    std::iter::repeat_n(letter, count as usize).collect()
}

/// Marques d'octave de `\fixed c'` pour une octave MIDI (do central = 4).
fn octave_marks(octave: i32) -> String {
    if octave >= 4 {
        "'".repeat((octave - 4) as usize)
    } else {
        ",".repeat((4 - octave) as usize)
    }
}

/// Nom orthographié d'une note, sans durée ni octave.
///
/// `transpose_chromatic` est la transposition de l'instrument, au sens de
/// `Note::pitch - transpose` (convention de `pitch2xml` dans
/// `exportmusicxml.cpp`) : la hauteur écrite est celle que voit le musicien.
/// Le `tpc` transposé est préféré quand il est valide.
pub(crate) fn pitch_name(note: &Note, transpose_chromatic: i32) -> String {
    let used_tpc = if valid(note.tpc_transposed) {
        note.tpc_transposed
    } else if valid(note.tpc) {
        note.tpc
    } else {
        return midi_name(note.pitch);
    };
    let written = i32::from(note.pitch) - transpose_chromatic;
    let alter = alter(used_tpc);
    // Même calcul que `written_pitch` : l'octave se déduit de la hauteur
    // écrite diminuée de l'altération.
    let octave = (written - alter).div_euclid(12) - 1;
    format!(
        "{}{}{}",
        step(used_tpc),
        alter_suffix(alter),
        octave_marks(octave)
    )
}

/// `\transposition <hauteur>` pour une transposition d'instrument.
///
/// LilyPond ne transpose que le MIDI : la hauteur écrite reste celle de la
/// partition, exactement comme le `<transpose>` de MuseScore. L'argument est
/// la hauteur qui sonne quand on écrit `c'` : LilyPond retranche alors
/// `72 - hauteur` demi-tons aux hauteurs MIDI. Ainsi `c` (60) donne -12,
/// `bf` (70) donne -2, `c'` (72) ne donne rien — la règle utilisée par
/// `musicxml2ly` pour la commande `\transposition`. `None` si la transposition
/// est nulle ou hors domaine MIDI.
pub(crate) fn transposition_command(transpose_chromatic: i32) -> Option<String> {
    if transpose_chromatic == 0 {
        return None;
    }
    let pitch = 72 + transpose_chromatic;
    if !(0..=127).contains(&pitch) {
        return None;
    }
    Some(format!("\\transposition {}", transposition_name(pitch)))
}

/// Nom d'une hauteur pour `\transposition`, avec l'orthographe des bémols
/// utilisée par `musicxml2ly` (`bf` et non `as`, `ef` et non `ds`).
fn transposition_name(pitch: i32) -> String {
    const NOMS: [&str; 12] = [
        "c", "cs", "d", "ef", "e", "f", "fs", "g", "af", "a", "bf", "b",
    ];
    let octave = pitch.div_euclid(12) - 1;
    format!("{}{}", NOMS[(pitch % 12) as usize], octave_marks(octave))
}

/// Nom déduit de la seule hauteur MIDI, quand aucun `tpc` n'est valide.
fn midi_name(pitch: u8) -> String {
    const STEPS: [char; 12] = ['c', 'c', 'd', 'd', 'e', 'f', 'f', 'g', 'g', 'a', 'a', 'b'];
    const ALTERS: [i32; 12] = [0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0];
    let index = usize::from(pitch % 12);
    let octave = i32::from(pitch) / 12 - 1;
    format!(
        "{}{}{}",
        STEPS[index],
        alter_suffix(ALTERS[index]),
        octave_marks(octave)
    )
}

/// Noms des toniques majeures, indexés par nombre de quintes (`-7..=7`).
const MAJOR_TONICS: [&str; 15] = [
    "cf", "gf", "df", "af", "ef", "bf", "f", "c", "g", "d", "a", "e", "b", "fs", "cs",
];

/// Noms des toniques mineures relatives, indexés par nombre de quintes.
const MINOR_TONICS: [&str; 15] = [
    "af", "ef", "bf", "f", "c", "g", "d", "a", "e", "b", "fs", "cs", "gs", "ds", "as",
];

/// Nom de tonique pour une armure, borné à `-7..=7` quintes.
fn tonic(fifths: i8, minor: bool) -> &'static str {
    let clamped = i32::from(fifths).clamp(-7, 7);
    let index = (clamped + 7) as usize;
    if minor {
        MINOR_TONICS[index]
    } else {
        MAJOR_TONICS[index]
    }
}

/// `\key <tonique> \major` ou `\minor` pour une armure.
///
/// Une armure sans mode (`KeyMode::None`) est écrite en majeur : LilyPond n'a
/// pas d'armure atonale et une tonalité majeure garde au moins les bonnes
/// altérations. Au-delà de sept quintes (armures personnalisées de MuseScore),
/// la tonalité est bornée à la plus proche armure standard.
pub(crate) fn key_signature(key: &KeySig) -> String {
    let minor = matches!(key.mode, KeyMode::Minor);
    let name = tonic(key.concert_key.fifths(), minor);
    if minor {
        format!("\\key {name} \\minor")
    } else {
        format!("\\key {name} \\major")
    }
}

/// Nom de clé LilyPond d'une [`Clef`].
///
/// Les clés octaviées s'écrivent entre guillemets (`"treble_8"`) ; les autres
/// sont des commandes prédéfinies. `None` pour une clef invalide.
pub(crate) fn clef_name(clef: &Clef) -> Option<String> {
    let name = clefs::name(clef.transposed)?;
    // Les noms octaviés et `tab` sont des chaînes pour LilyPond : `\clef
    // "bass_8"`, `\clef "tab"`. Les autres sont des commandes prédéfinies.
    Some(if needs_quotes(name) {
        format!("\"{name}\"")
    } else {
        name.to_owned()
    })
}

/// Vrai si un nom de clef doit être écrit entre guillemets.
fn needs_quotes(name: &str) -> bool {
    !name.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// `\clef …` d'une clef, ou `None` si elle n'est pas représentable.
pub(crate) fn clef_command(clef: &Clef) -> Option<String> {
    clef_name(clef).map(|name| format!("\\clef {name}"))
}

/// Style de tête de note LilyPond, quand il diffère du style normal.
///
/// Reprend la table de `musicxml2ly` (`NotestyleEvent.notehead_styles_dict`) :
/// `\tweak style #'cross`, `#'harmonic-mixed`, `#'do`…
pub(crate) fn note_head_style(group: &NoteHeadGroup) -> Option<&'static str> {
    match group {
        NoteHeadGroup::Cross | NoteHeadGroup::HeavyCross | NoteHeadGroup::HeavyCrossHat => {
            Some("cross")
        }
        NoteHeadGroup::Diamond | NoteHeadGroup::DiamondOld | NoteHeadGroup::LargeDiamond => {
            Some("harmonic-mixed")
        }
        NoteHeadGroup::TriangleUp | NoteHeadGroup::TriangleDown | NoteHeadGroup::LargeArrow => {
            Some("do")
        }
        NoteHeadGroup::Slash | NoteHeadGroup::Slashed1 | NoteHeadGroup::Slashed2 => Some("slash"),
        NoteHeadGroup::Circled | NoteHeadGroup::CircledLarge | NoteHeadGroup::XCircle => {
            Some("xcircle")
        }
        NoteHeadGroup::Do => Some("do"),
        NoteHeadGroup::Re => Some("re"),
        NoteHeadGroup::Mi => Some("mi"),
        NoteHeadGroup::Fa => Some("fa"),
        NoteHeadGroup::Sol => Some("sol"),
        NoteHeadGroup::La => Some("la"),
        NoteHeadGroup::Ti => Some("ti"),
        NoteHeadGroup::Normal
        | NoteHeadGroup::Plus
        | NoteHeadGroup::WithX
        | NoteHeadGroup::BrevisAlt
        | NoteHeadGroup::Other(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoretk_core::model::{tpc, ClefType, Key};

    #[test]
    fn degres_et_altered() {
        assert_eq!(step(tpc::C), 'c');
        assert_eq!(step(tpc::F), 'f');
        assert_eq!(step(tpc::B), 'b');
        assert_eq!(alter(tpc::C), 0);
        assert_eq!(alter(tpc::C_SHARP), 1);
        assert_eq!(alter(tpc::B_FLAT), -1);
        assert_eq!(alter(tpc::F_SHARP), 1);
        assert_eq!(alter(tpc::C_FLAT), -1);
        assert_eq!(alter_suffix(-2), "ff");
        assert_eq!(alter_suffix(2), "ss");
        assert_eq!(alter_suffix(0), "");
    }

    #[test]
    fn octaves_du_mode_fixed() {
        assert_eq!(octave_marks(4), "");
        assert_eq!(octave_marks(5), "'");
        assert_eq!(octave_marks(3), ",");
        assert_eq!(octave_marks(2), ",,");
    }

    #[test]
    fn noms_de_notes() {
        assert_eq!(pitch_name(&Note::new(60, tpc::C), 0), "c");
        assert_eq!(pitch_name(&Note::new(61, tpc::C_SHARP), 0), "cs");
        assert_eq!(pitch_name(&Note::new(70, tpc::B_FLAT), 0), "bf");
        assert_eq!(pitch_name(&Note::new(48, tpc::C), 0), "c,");
        // Fa bémol 4 : la hauteur MIDI est celle du mi 4.
        assert_eq!(pitch_name(&Note::new(64, tpc::F_FLAT), 0), "ff");
        // Clarinette en si bémol : la note écrite est une seconde au-dessus
        // (do 4 sonne ré 4).
        let mut note = Note::new(60, tpc::C);
        note.tpc_transposed = tpc::D;
        assert_eq!(pitch_name(&note, -2), "d");
    }

    #[test]
    fn transposition_des_instruments() {
        assert_eq!(transposition_command(0), None);
        assert_eq!(transposition_command(-12).as_deref(), Some("\\transposition c"));
        assert_eq!(transposition_command(-2).as_deref(), Some("\\transposition bf"));
        assert_eq!(transposition_command(2).as_deref(), Some("\\transposition d'"));
        assert_eq!(transposition_command(-7).as_deref(), Some("\\transposition f"));
        assert_eq!(transposition_command(-200), None);
    }

    #[test]
    fn armures() {
        assert_eq!(key_signature(&KeySig::new(Key::C)), "\\key c \\major");
        let mut key = KeySig::new(Key::new(-2));
        key.mode = KeyMode::Minor;
        assert_eq!(key_signature(&key), "\\key g \\minor");
        let mut key = KeySig::new(Key::new(6));
        key.mode = KeyMode::Minor;
        assert_eq!(key_signature(&key), "\\key ds \\minor");
    }

    #[test]
    fn cles() {
        assert_eq!(
            clef_command(&Clef::new(ClefType::G)).as_deref(),
            Some("\\clef treble")
        );
        assert_eq!(
            clef_command(&Clef::new(ClefType::F8Vb)).as_deref(),
            Some("\\clef \"bass_8\"")
        );
        assert_eq!(clef_command(&Clef::new(ClefType::Invalid)), None);
    }
}
