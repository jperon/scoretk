// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les noms de notes, d'armures et de clés suivent la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>) et le comportement de
// `xml2abc.py` et `abc2xml.py` de Willem Vree. Aucune ligne de ces scripts
// n'est copiée : la mise en œuvre est une réécriture Rust.
//
// Le calcul des degrés et des altérations reprend `tpc2stepName` et
// `tpc2alter` de `engraving/dom/pitchspelling.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Hauteurs, armures et clés ABC.
//!
//! Le modèle porte deux informations de hauteur : [`Note::pitch`], la hauteur
//! MIDI, et [`Note::tpc`], la classe de hauteur tonale qui fixe l'orthographe
//! (do dièse plutôt que ré bémol). ABC distingue les deux par la lettre et
//! l'altération explicite (`^C` pour do dièse).
//!
//! # Octaves
//!
//! ABC écrit `C` pour le do central (octave 4), `c` pour l'octave 5, `C,`
//! pour l'octave 3 et `c'` pour l'octave 6. [`octave_marks`] produit les
//! virgules et apostrophes, [`abc_note`] applique la casse.
//!
//! # Armures et modes
//!
//! Le modèle ne connaît que majeur, mineur, aucun mode et inconnu. Les modes
//! ABC (dorian, phrygien, lydien, mixolydien, éolien, locrien) sont donc
//! ramenés à l'armure correspondante, avec un avertissement à la lecture.

use scoretk_core::model::{Clef, ClefType, Key, KeyMode, KeySig, Note};

/// Plus petite valeur de `tpc` valide (`Tpc::TPC_MIN`, fa triple bémol).
const TPC_MIN: i32 = -8;

/// Degrés de la spirale des quintes, comme `tpc2stepName`.
const SPIRAL_STEPS: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];

/// Demi-tons naturels de chaque lettre (do = 0).
const NATURAL_PC: [(char, i32); 7] = [
    ('C', 0),
    ('D', 2),
    ('E', 4),
    ('F', 5),
    ('G', 7),
    ('A', 9),
    ('B', 11),
];

/// Quintes de la gamme majeure de chaque tonique naturelle.
const MAJOR_FIFTHS: [(char, i32); 7] = [
    ('C', 0),
    ('D', 2),
    ('E', 4),
    ('F', -1),
    ('G', 1),
    ('A', 3),
    ('B', 5),
];

/// Toniques majeures indexées par nombre de quintes (`-7..=7`).
const MAJOR_TONICS: [&str; 15] = [
    "Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#",
];

/// Toniques mineures relatives indexées par nombre de quintes.
const MINOR_TONICS: [&str; 15] = [
    "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#",
];

/// Ordre des dièses à l'armure.
const SHARP_ORDER: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];

/// Ordre des bémols à l'armure.
const FLAT_ORDER: [char; 7] = ['B', 'E', 'A', 'D', 'G', 'C', 'F'];

/// Vrai si le `tpc` est exploitable (`tpc::INVALID` sinon).
pub fn valid_tpc(value: i8) -> bool {
    (TPC_MIN..=40).contains(&i32::from(value))
}

/// Lettre ABC (majuscule) d'un `tpc`.
pub fn letter(tpc_value: i8) -> char {
    let index = (i32::from(tpc_value) - TPC_MIN).rem_euclid(7) as usize;
    SPIRAL_STEPS[index]
}

/// Altération en demi-tons d'un `tpc`, comme `tpc2alter`.
pub fn alter(tpc_value: i8) -> i32 {
    (i32::from(tpc_value) - TPC_MIN) / 7 - 3
}

/// `tpc` correspondant à une lettre et une altération.
pub fn tpc_of(letter: char, alter: i32) -> i8 {
    let index = SPIRAL_STEPS
        .iter()
        .position(|candidate| *candidate == letter.to_ascii_uppercase())
        .unwrap_or(0) as i32;
    (7 * (alter + 3) + index - 8) as i8
}

/// Octave scientifique d'une hauteur écrite.
fn octave_of(written_pitch: i32, alter: i32) -> i32 {
    (written_pitch - alter).div_euclid(12) - 1
}

/// Virgules et apostrophes ABC pour une octave (do central = 4).
pub fn octave_marks(octave: i32) -> String {
    if octave >= 5 {
        "'".repeat((octave - 5) as usize)
    } else {
        ",".repeat((4 - octave).max(0) as usize)
    }
}

/// Casse ABC d'une octave : majuscule jusqu'à 4, minuscule au-delà.
fn octave_case(octave: i32) -> bool {
    octave >= 5
}

/// Nom ABC d'une note, sans altération explicite.
///
/// Renvoie la lettre, l'octave et l'altération absolue lue dans le `tpc` (ou
/// déduite de la hauteur MIDI quand aucun `tpc` n'est valide).
pub fn note_spelling(note: &Note, transpose_chromatic: i32) -> (char, i32, i32) {
    let written = i32::from(note.pitch) - transpose_chromatic;
    if valid_tpc(note.tpc_transposed) {
        (
            letter(note.tpc_transposed),
            alter(note.tpc_transposed),
            octave_of(written, alter(note.tpc_transposed)),
        )
    } else if valid_tpc(note.tpc) {
        (
            letter(note.tpc),
            alter(note.tpc),
            octave_of(written, alter(note.tpc)),
        )
    } else {
        midi_spelling(written)
    }
}

/// Orthographe de repli déduite de la hauteur MIDI (dièses).
fn midi_spelling(pitch: i32) -> (char, i32, i32) {
    const LETTERS: [char; 12] = ['C', 'C', 'D', 'D', 'E', 'F', 'F', 'G', 'G', 'A', 'A', 'B'];
    const ALTERS: [i32; 12] = [0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0];
    let index = pitch.rem_euclid(12) as usize;
    let alter = ALTERS[index];
    (LETTERS[index], alter, octave_of(pitch, alter))
}

/// Nom ABC d'une note, altération explicite comprise.
///
/// `alter` est l'altération absolue voulue ; elle est écrite avec `^`, `_` ou
/// `=` quand elle diffère de l'état courant (`state`), qui suit les altérations
/// déjà posées dans la mesure et l'armure. `state` est mis à jour.
pub fn abc_note(letter: char, alter: i32, octave: i32, state: &mut AccidentalState) -> String {
    let mut token = String::new();
    if state.current(letter) != alter {
        token.push_str(&accidental_token(alter));
        state.set(letter, alter);
    }
    let lower = letter.to_ascii_lowercase();
    token.push(if octave_case(octave) {
        lower
    } else {
        lower.to_ascii_uppercase()
    });
    token.push_str(&octave_marks(octave));
    token
}

/// Jeton d'altération absolue (`^^`, `^`, `=`, `_`, `__`).
fn accidental_token(alter: i32) -> String {
    match alter {
        0 => "=".to_owned(),
        value if value > 0 => "^".repeat(value as usize),
        value => "_".repeat((-value) as usize),
    }
}

/// État des altérations d'une mesure.
///
/// ABC mémorise l'altération d'une lettre jusqu'à la barre suivante. L'état
/// part de l'armure, puis suit les altérations explicites écrites.
#[derive(Clone, Debug)]
pub struct AccidentalState {
    values: [i32; 7],
}

impl AccidentalState {
    /// État initial d'une mesure pour une armure.
    pub fn new(fifths: i8) -> Self {
        let mut values = [0; 7];
        for (index, letter) in ['C', 'D', 'E', 'F', 'G', 'A', 'B'].iter().enumerate() {
            values[index] = key_alter(fifths, *letter);
        }
        Self { values }
    }

    /// Altération courante d'une lettre.
    pub fn current(&self, letter: char) -> i32 {
        self.values[letter_index(letter)]
    }

    /// Fixe l'altération courante d'une lettre.
    pub fn set(&mut self, letter: char, alter: i32) {
        self.values[letter_index(letter)] = alter;
    }
}

/// Indice d'une lettre (`C` = 0).
fn letter_index(letter: char) -> usize {
    match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        _ => 6,
    }
}

/// Altération imposée par l'armure à une lettre.
pub fn key_alter(fifths: i8, letter: char) -> i32 {
    let letter = letter.to_ascii_uppercase();
    if fifths > 0 {
        if SHARP_ORDER
            .iter()
            .take(fifths as usize)
            .any(|candidate| *candidate == letter)
        {
            1
        } else {
            0
        }
    } else if fifths < 0 {
        if FLAT_ORDER
            .iter()
            .take((-fifths) as usize)
            .any(|candidate| *candidate == letter)
        {
            -1
        } else {
            0
        }
    } else {
        0
    }
}

/// Nom de tonique pour une armure, bornée à `-7..=7` quintes.
fn tonic(fifths: i8, minor: bool) -> &'static str {
    let clamped = i32::from(fifths).clamp(-7, 7);
    let index = (clamped + 7) as usize;
    if minor {
        MINOR_TONICS[index]
    } else {
        MAJOR_TONICS[index]
    }
}

/// Champ `K:` d'une armure.
pub fn key_field(key: &KeySig) -> String {
    match key.mode {
        KeyMode::None => "none".to_owned(),
        KeyMode::Minor => format!("{}m", tonic(key.transposed_key.fifths(), true)),
        KeyMode::Major | KeyMode::Unknown => tonic(key.transposed_key.fifths(), false).to_owned(),
    }
}

/// Quintes d'une gamme majeure à partir d'une lettre et d'une altération.
fn major_fifths(letter: char, alter: i32) -> Option<i32> {
    let base = MAJOR_FIFTHS
        .iter()
        .find(|(candidate, _)| *candidate == letter)
        .map(|(_, fifths)| *fifths)?;
    Some(base + 7 * alter)
}

/// Position d'une lettre dans la gamme (`C` = 0, … `B` = 6).
fn diatonic_index(letter: char) -> i32 {
    match letter {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        _ => 6,
    }
}

/// Lettre à un décalage diatonique.
fn shifted_letter(letter: char, offset: i32) -> char {
    const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    LETTERS[(diatonic_index(letter) + offset).rem_euclid(7) as usize]
}

/// Classe de hauteur d'une lettre naturelle.
fn natural_pc(letter: char) -> i32 {
    NATURAL_PC
        .iter()
        .find(|(candidate, _)| *candidate == letter)
        .map_or(0, |(_, pc)| *pc)
}

/// Mode ABC reconnu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
}

impl Mode {
    /// Décalage diatonique (en lettres) vers la majeure relative.
    fn letter_offset(self) -> i32 {
        match self {
            Self::Ionian => 0,
            Self::Dorian => 1,
            Self::Phrygian => 2,
            Self::Lydian => 3,
            Self::Mixolydian => 4,
            Self::Aeolian => 5,
            Self::Locrian => 6,
        }
    }

    /// Décalage en demi-tons vers la majeure relative.
    fn semitone_offset(self) -> i32 {
        match self {
            Self::Ionian => 0,
            Self::Dorian => 2,
            Self::Phrygian => 4,
            Self::Lydian => 5,
            Self::Mixolydian => 7,
            Self::Aeolian => 9,
            Self::Locrian => 11,
        }
    }
}

/// Résultat de l'analyse d'un champ `K:`.
pub struct ParsedKey {
    /// Armure.
    pub key: KeySig,
    /// Mode non majeur/mineur, à signaler.
    pub exotic_mode: Option<&'static str>,
    /// Paramètres du champ (`clef=`, `octave=`…).
    pub parameters: Vec<(String, String)>,
}

/// Vrai si le mot est un nom de mode ABC (éventuellement abrégé).
fn looks_like_mode(word: &str) -> bool {
    let lowered = word.to_ascii_lowercase();
    lowered == "m"
        || [
            "maj", "min", "aeo", "ion", "dor", "phr", "lyd", "mix", "loc",
        ]
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
}

/// Analyse un champ `K:` (valeur après `K:`).
pub fn parse_key(text: &str) -> Option<ParsedKey> {
    let mut parts = text.split_whitespace();
    let first = parts.next()?;
    // Un champ ne contenant que des paramètres (`K:clef=treble-8`,
    // `[K:octave=-1]`) n'a pas de tonique. `abc2xml.py::doClef` (1736-1744)
    // lit quand même `clef=` ; ABC 2.1 §4.6 (spec 921-933) donne à ce
    // paramètre le même sens que dans un champ avec tonique.
    let first_name = first
        .split_once('=')
        .map(|(name, _)| name.to_ascii_lowercase());
    if first_name.as_deref().is_some_and(|name| {
        matches!(
            name,
            "clef" | "transpose" | "octave" | "stafflines" | "middle" | "m"
        )
    }) {
        let parameters = std::iter::once(first)
            .chain(parts)
            .filter_map(|part| {
                let (name, value) = part.split_once('=')?;
                Some((name.to_ascii_lowercase(), value.to_owned()))
            })
            .collect();
        return Some(ParsedKey {
            key: KeySig::new(Key::C),
            exotic_mode: None,
            parameters,
        });
    }
    // Port de `abc2xml.py` 269, `doHeaderField`, branche `K:` (lignes 1808-1820)
    // : la regex `([A-G][#b]?)\s*([a-zA-Z]*)` lit la tonique **et** le mot de
    // mode qui la suit, même séparés par un espace. `K:E Minor` est donc
    // une armure de 1 dièse (mineur relatif de E), et non 4 dièses (E majeur).
    // Un mot non reconnu comme mode est traité comme majeur par le script
    // (ligne 1817) : on le laisse alors aux paramètres, comme `clef=treble`.
    let mut tonic_text = first.to_owned();
    let mut rest = Vec::new();
    if let Some(next) = parts.next() {
        let word: String = next
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if looks_like_mode(&word) {
            tonic_text.push_str(&word);
        } else {
            rest.push(next);
        }
    }
    rest.extend(parts);
    let parameters = rest
        .into_iter()
        .filter_map(|part| {
            let (name, value) = part.split_once('=')?;
            Some((name.to_ascii_lowercase(), value.to_owned()))
        })
        .collect();

    let lowered = tonic_text.to_ascii_lowercase();
    if lowered == "none" {
        return Some(ParsedKey {
            key: KeySig {
                concert_key: Key::C,
                transposed_key: Key::C,
                mode: KeyMode::None,
                courtesy: false,
                visible: true,
            },
            exotic_mode: None,
            parameters,
        });
    }
    if lowered == "hp" {
        return Some(ParsedKey {
            key: KeySig::new(Key::D),
            exotic_mode: None,
            parameters,
        });
    }

    let mut chars = tonic_text.chars();
    // La tonique doit être une lettre **majuscule** (`abc2xml.py`,
    // `doHeaderField`, regex ligne 1811) : un champ `K: bass-8`/`K: treble+8`
    // ne porte que des paramètres et laisse C majeur par défaut.
    let first_letter = chars.next()?;
    if !first_letter.is_ascii_uppercase() {
        return None;
    }
    let letter = first_letter.to_ascii_uppercase();
    if !('A'..='G').contains(&letter) {
        return None;
    }
    let mut alter = 0i32;
    let mut rest = String::new();
    for character in chars {
        match character {
            // `abc2xml.py::doHeaderField` (ligne 1811) lit la tonique avec
            // `([A-G][#b]?)` : les altérations explicites collées à la
            // tonique (`K:Bb^f`) ne font pas partie du nom de la tonique et
            // sont ignorées (ABC 2.1 §3.1.14). Les traiter comme un `#`
            // changeait `Bb` (2 bémols) en `B` (5 dièses).
            '^' | '_' | '=' => break,
            '#' => alter += 1,
            'b' => alter -= 1,
            _ => rest.push(character),
        }
    }
    let suffix = rest.to_ascii_lowercase();
    // Port de `abc2xml.py::doHeaderField` (lignes 1815-1817) : seul le
    // préfixe de trois lettres du mode est significatif, et un préfixe
    // inconnu est traité comme majeur (`if mode not in s.offTab: mode='maj'`).
    let court: String = suffix.chars().take(3).collect();
    let mode = match court.as_str() {
        "" | "maj" | "ion" => Mode::Ionian,
        "m" | "min" | "aeo" => Mode::Aeolian,
        "dor" => Mode::Dorian,
        "phr" => Mode::Phrygian,
        "lyd" => Mode::Lydian,
        "mix" => Mode::Mixolydian,
        "loc" => Mode::Locrian,
        _ => Mode::Ionian,
    };

    let major_letter = shifted_letter(letter, -mode.letter_offset());
    let tonic_pc = (natural_pc(letter) + alter).rem_euclid(12);
    let major_pc = (tonic_pc + 12 - mode.semitone_offset()).rem_euclid(12);
    let natural = natural_pc(major_letter);
    let mut major_alter = (major_pc - natural).rem_euclid(12);
    if major_alter > 6 {
        major_alter -= 12;
    }
    let fifths = major_fifths(major_letter, major_alter)?;

    let model_mode = match mode {
        Mode::Ionian => KeyMode::Major,
        Mode::Aeolian => KeyMode::Minor,
        _ => KeyMode::Major,
    };
    let exotic_mode = match mode {
        Mode::Ionian | Mode::Aeolian => None,
        Mode::Dorian => Some("dorian"),
        Mode::Phrygian => Some("phrygien"),
        Mode::Lydian => Some("lydien"),
        Mode::Mixolydian => Some("mixolydien"),
        Mode::Locrian => Some("locrien"),
    };
    Some(ParsedKey {
        key: KeySig {
            concert_key: Key::new(fifths as i8),
            transposed_key: Key::new(fifths as i8),
            mode: model_mode,
            courtesy: false,
            visible: true,
        },
        exotic_mode,
        parameters,
    })
}

/// Nom ABC d'une clé.
pub fn clef_name(clef: &Clef) -> Option<String> {
    let name = match clef.transposed {
        ClefType::G => "treble",
        ClefType::G8Vb => "treble-8",
        ClefType::G8Va => "treble+8",
        ClefType::G15Mb => "treble-15",
        ClefType::G15Ma => "treble+15",
        ClefType::G1 => "french",
        ClefType::C1 => "soprano",
        ClefType::C2 => "mezzosoprano",
        ClefType::C3 => "alto",
        ClefType::C4 => "tenor",
        ClefType::C5 => "baritone",
        ClefType::F => "bass",
        ClefType::F8Vb => "bass-8",
        ClefType::F8Va => "bass+8",
        ClefType::F15Mb => "bass-15",
        ClefType::F15Ma => "bass+15",
        ClefType::FBaritone => "varbaritone",
        ClefType::FSubbass => "subbass",
        ClefType::Perc | ClefType::Perc2 => "perc",
        ClefType::Tab | ClefType::Tab4 | ClefType::TabSerif | ClefType::Tab4Serif => "tab",
        ClefType::Invalid => return None,
    };
    Some(name.to_owned())
}

/// Clé correspondant à un nom ABC.
pub fn parse_clef(text: &str) -> Option<ClefType> {
    let lowered = text.trim().to_ascii_lowercase();
    let (base, octave) = if let Some(rest) = lowered.strip_suffix("-8") {
        (rest, -1)
    } else if let Some(rest) = lowered.strip_suffix("+8") {
        (rest, 1)
    } else if let Some(rest) = lowered.strip_suffix("-15") {
        (rest, -2)
    } else if let Some(rest) = lowered.strip_suffix("+15") {
        (rest, 2)
    } else {
        (lowered.as_str(), 0)
    };
    let clef = match base {
        "treble" | "g" | "violin" => match octave {
            -1 => ClefType::G8Vb,
            1 => ClefType::G8Va,
            -2 => ClefType::G15Mb,
            2 => ClefType::G15Ma,
            _ => ClefType::G,
        },
        "french" | "g1" => ClefType::G1,
        "soprano" | "c1" => ClefType::C1,
        "mezzosoprano" | "mezzo-soprano" | "c2" => ClefType::C2,
        "alto" | "c3" => ClefType::C3,
        "tenor" | "c4" => ClefType::C4,
        "baritone" | "c5" => ClefType::C5,
        "bass" | "f" => match octave {
            -1 => ClefType::F8Vb,
            1 => ClefType::F8Va,
            -2 => ClefType::F15Mb,
            2 => ClefType::F15Ma,
            _ => ClefType::F,
        },
        "varbaritone" => ClefType::FBaritone,
        "subbass" => ClefType::FSubbass,
        "perc" | "percussion" => ClefType::Perc,
        "tab" | "tablature" => ClefType::Tab,
        _ => return None,
    };
    Some(clef)
}

#[cfg(test)]
mod tests {
    #[test]
    fn clef_seule_n_est_pas_une_tonique() {
        // `abc2xml.py::doHeaderField` (ligne 1811, regex
        // `([A-G][#b]?)\s*([a-zA-Z]*)`) n'accepte qu'une tonique majuscule :
        // `K: bass-8` est un champ de clef, pas une armure en si bémol.
        assert!(parse_key("bass-8").is_none());
        assert!(parse_key("treble+8").is_none());
        assert!(parse_key("C clef=bass").is_some());
    }

    use super::*;
    use scoretk_core::model::{tpc, Note};

    #[test]
    fn degres_et_altered() {
        assert_eq!(letter(tpc::C), 'C');
        assert_eq!(letter(tpc::F), 'F');
        assert_eq!(letter(tpc::B), 'B');
        assert_eq!(alter(tpc::C), 0);
        assert_eq!(alter(tpc::C_SHARP), 1);
        assert_eq!(alter(tpc::B_FLAT), -1);
        assert_eq!(alter(tpc::F_SHARP), 1);
        assert_eq!(alter(tpc::C_FLAT), -1);
        assert_eq!(tpc_of('C', 0), tpc::C);
        assert_eq!(tpc_of('C', 1), tpc::C_SHARP);
        assert_eq!(tpc_of('B', -1), tpc::B_FLAT);
    }

    #[test]
    fn octaves_abc() {
        assert_eq!(octave_marks(4), "");
        assert_eq!(octave_marks(5), "");
        assert_eq!(octave_marks(6), "'");
        assert_eq!(octave_marks(3), ",");
        assert_eq!(octave_marks(2), ",,");
    }

    #[test]
    fn nom_de_note() {
        let mut state = AccidentalState::new(0);
        assert_eq!(abc_note('C', 0, 4, &mut state), "C");
        assert_eq!(abc_note('C', 0, 5, &mut state), "c");
        let mut state = AccidentalState::new(0);
        assert_eq!(abc_note('F', 1, 4, &mut state), "^F");
        // L'altération est mémorisée dans la mesure.
        assert_eq!(abc_note('F', 1, 4, &mut state), "F");
        assert_eq!(abc_note('F', 0, 4, &mut state), "=F");
    }

    #[test]
    fn armures_et_modes() {
        let c = KeySig::new(Key::C);
        assert_eq!(key_field(&c), "C");
        let mut g = KeySig::new(Key::G);
        g.mode = KeyMode::Minor;
        assert_eq!(key_field(&g), "Em");
        let mut none = KeySig::new(Key::C);
        none.mode = KeyMode::None;
        assert_eq!(key_field(&none), "none");

        let parsed = parse_key("Bb").expect("si bémol majeur");
        assert_eq!(parsed.key.concert_key.fifths(), -2);
        let parsed = parse_key("F#m").expect("fa dièse mineur");
        assert_eq!(parsed.key.concert_key.fifths(), 3);
        assert_eq!(parsed.key.mode, KeyMode::Minor);
        let parsed = parse_key("Ddor").expect("ré dorien");
        assert_eq!(parsed.key.concert_key.fifths(), 0);
        assert_eq!(parsed.exotic_mode, Some("dorian"));
        let parsed = parse_key("C clef=bass octave=-1").expect("paramètres");
        assert_eq!(parsed.parameters[0].0, "clef");
        assert_eq!(parsed.parameters[1].1, "-1");
        assert!(parse_key("H").is_none());
    }

    #[test]
    fn cles() {
        assert_eq!(
            clef_name(&Clef::new(ClefType::G)).as_deref(),
            Some("treble")
        );
        assert_eq!(
            clef_name(&Clef::new(ClefType::F8Vb)).as_deref(),
            Some("bass-8")
        );
        assert_eq!(parse_clef("bass-8"), Some(ClefType::F8Vb));
        assert_eq!(parse_clef("treble+8"), Some(ClefType::G8Va));
        assert_eq!(parse_clef("alto"), Some(ClefType::C3));
        assert_eq!(parse_clef("inconnue"), None);
    }

    #[test]
    fn armure_par_lettre() {
        assert_eq!(key_alter(2, 'F'), 1);
        assert_eq!(key_alter(2, 'C'), 1);
        assert_eq!(key_alter(2, 'G'), 0);
        assert_eq!(key_alter(-3, 'B'), -1);
        assert_eq!(key_alter(-3, 'E'), -1);
        assert_eq!(key_alter(-3, 'A'), -1);
        assert_eq!(key_alter(-3, 'D'), 0);
    }

    #[test]
    fn note_sans_tpc() {
        let mut note = Note::new(61, tpc::INVALID);
        note.tpc_transposed = tpc::INVALID;
        assert_eq!(note_spelling(&note, 0), ('C', 1, 4));
    }
}
