// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : tables de correspondance entre le flux L6
// grégorien (GABC) et le module NEUMES de MEI 5.1, d'après les *Guidelines*
// ch. 6 (« Repertoire: Neume Notation », §6.1, §6.1.1, §6.4.1–§6.4.10) et le
// schéma normatif `mei-all-5.1.rng` (ECL-2.0), rédigées en 2026. Les règles
// GABC sont celles de Gregorio `doc/Gabc.tex:408-489,801-870`, relu comme
// spécification et réécrit en Rust en 2026.
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

//! Correspondances GABC ↔ MEI 5.1 Neume Notation.
//!
//! Les attributs standards sont utilisés partout où MEI 5.1 a un porteur :
//! `<clef shape="C|F" line>`, `<nc @pname @oct @tilt @curve>`, `<oriscus>`,
//! `<quilisma>`, `<liquescent>`, `<strophicus>`, `<episema form="h|v">` et
//! `<divLine form>` (`Guidelines` §6.4.1–§6.4.7). Les jetons **atomiques** de
//! GABC qui n'ont pas de champ MEI (orthographe exacte d'un glyphe, chiffres
//! de placement, clé double `c1@c4`, barres dominicaines, en-têtes) sont
//! transportés par des `<annot type="gabc-*" n="…">` : MEI 5.1 définit `@type`
//! comme un type libre (`att.typed`) et `<annot>` comme un conteneur de
//! déclaration (`Guidelines` §3, `mei_annot`). Ce sont des extensions
//! scoretk, au même titre que les `@type="mscore-*"` de MuseScore ; un lecteur
//! MEI tiers peut les ignorer et lire le reste de l'encodage neumatique.
//!
//! La convention d'octave est celle des autres directions (MuseScore 4.7.4,
//! `format-ref/mei.md` §7) : do moyen = `c`/`4` (`Convert::pitchFromMEI`).

use scoretk_core::model::{BarKind, SignKind};

/// `<annot type="…">` portant un en-tête GABC (`n` = nom du champ).
pub const HEADER_ANNOT: &str = "gabc-header";
/// `<annot type="…">` portant un jeton de clé atomique.
pub const CLEF_ANNOT: &str = "gabc-clef";
/// `<annot type="…">` portant `lettre + glyphe` d'une note.
pub const NOTE_ANNOT: &str = "gabc-note";
/// `<annot type="…">` portant le jeton exact d'un signe.
pub const SIGN_ANNOT: &str = "gabc-sign";
/// `<annot type="…">` portant le jeton exact d'une barre de séparation.
pub const BAR_ANNOT: &str = "gabc-bar";
/// `<annot type="…">` portant le texte au-dessus de la portée.
pub const ABOVE_ANNOT: &str = "gabc-above";
/// `<annot type="…">` portant la rubrique `<sp>`/`<alt>`.
pub const RUBRIC_ANNOT: &str = "gabc-rubric";
/// `<annot type="…">` portant l'espace après le groupe.
pub const SPACE_ANNOT: &str = "gabc-space";
/// `<annot type="…">` portant `début:fin` en unités UTF-16.
pub const SOURCE_ANNOT: &str = "gabc-source";
/// Préfixe des annotations de signes attachées à une note (`n = note:signe`).
pub const NOTE_SIGN_ANNOT: &str = "gabc-note-sign";
/// Préfixe des annotations de signes attachées au groupe (`n = signe`).
pub const GROUP_SIGN_ANNOT: &str = "gabc-group-sign";
/// `<meiversion>` du module neumes : `mei-all`, pas `mei-basic`.
pub const NEUMES_VERSION: &str = "5.1";

/// Partie d'une clé GABC : forme `C` ou `F` et numéro de ligne.
///
/// `Gabc.tex:847-870` et `src/struct.c:1273-1288` : une clé `c3`/`f4` est
/// portée en MEI par `<clef shape="C|F" line="3|4">` (`Guidelines` §6.4.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeiClef {
    pub shape: char,
    pub line: u8,
}

/// Lit la première clé d'un jeton GABC (`cb3`, `c3@f4`, …).
pub fn clef_parts(token: &str) -> Option<MeiClef> {
    let first = token.split('@').next().unwrap_or(token);
    let shape = match first.as_bytes().first()? {
        b'c' | b'C' => 'C',
        b'f' | b'F' => 'F',
        _ => return None,
    };
    let line = first
        .bytes()
        .rev()
        .find(|byte| byte.is_ascii_digit())
        .unwrap_or(b'4')
        - b'0';
    (1..=9).contains(&line).then_some(MeiClef { shape, line })
}

/// Réécrit une clé MEI sous forme GABC canonique (première clé seulement).
pub fn clef_token(shape: &str, line: u8) -> String {
    let sign = if shape.eq_ignore_ascii_case("f") {
        'f'
    } else {
        'c'
    };
    format!("{sign}{line}")
}

/// Clé numérique de Gregorio (`src/struct.c:1273-1288`).
pub fn clef_key(token: &str) -> i32 {
    let first = token.split('@').next().unwrap_or(token);
    let line = i32::from(
        first
            .bytes()
            .rev()
            .find(|byte| byte.is_ascii_digit())
            .unwrap_or(b'4')
            - b'0',
    );
    if first.starts_with('f') {
        2 * line - 4
    } else {
        2 * line - 1
    }
}

/// Hauteur de portée GABC d'une lettre (`src/struct.h:809-813`).
pub fn staff_height(letter: char) -> i32 {
    let low = letter.to_ascii_lowercase();
    let base = low as i32 - 'a' as i32 + 3;
    if low > 'o' {
        base + 1
    } else {
        base
    }
}

/// Note GABC → degré diatonique (do = 0), relatif au do moyen.
pub fn degree(letter: char, clef: &str) -> i32 {
    staff_height(letter) - clef_key(clef) - 5
}

const PNAMES: [char; 7] = ['c', 'd', 'e', 'f', 'g', 'a', 'b'];
const SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Note GABC → (`@pname`, `@oct`) MEI 5.1.
///
/// Le do moyen (degré 0) est `c`/`4`, comme dans le reste du port
/// (`format-ref/mei.md` §7, piège 2). GABC n'a pas de hauteur absolue
/// (`Gabc.tex:410-413`) : cette octave est celle de la projection moderne.
pub fn nc_position(letter: char, clef: &str) -> (char, u8) {
    let step = degree(letter, clef);
    let index = step.rem_euclid(7) as usize;
    let pitch = 60 + SEMITONES[index] + 12 * step.div_euclid(7);
    let oct = (pitch - SEMITONES[index]) / 12 - 1;
    (PNAMES[index], oct.clamp(0, 9) as u8)
}

/// (`@pname`, `@oct`) MEI → lettre GABC sous la clé courante.
///
/// Réciproque de [`nc_position`] : `degré = 7 * (oct - 4) + index(pname)`,
/// puis `hauteur = degré + clé + 5` (`src/struct.h:809-813`). La lettre `o`
/// étant l'oriscus et non une note (`Gabc.tex:469`), elle est sautée.
pub fn nc_letter(pname: Option<&str>, oct: Option<u8>, clef: &str) -> Option<char> {
    let first = pname?.as_bytes().first().copied()?.to_ascii_lowercase();
    let index = PNAMES.iter().position(|name| *name as u8 == first)?;
    let oct = oct?;
    let height = (i32::from(oct) - 4) * 7 + index as i32 + clef_key(clef) + 5;
    // `a` = hauteur 3 ; `p` = hauteur 19 (le saut de `o`, qui est l'oriscus).
    let slot = if height >= 19 { height - 4 } else { height - 3 };
    let letter = char::from_u32(u32::try_from(slot).ok()? + u32::from('a'))?;
    (!matches!(letter, 'q'..='z')).then_some(letter)
}

/// Barre GABC → `@form` de `<divLine>` (`Guidelines` §6.4.1).
///
/// L'énumération suggérée est `caesura`, `finalis`, `maior`, `maxima`,
/// `minima`, `virgula` (`mei-all-5.1.rng`, `att.divLine.log/@form`) ; une
/// valeur NMTOKEN libre est autorisée, mais les valeurs suggérées suffisent
/// aux cinq bars de `Gabc.tex:808-817`.
pub fn bar_form(bar: BarKind) -> &'static str {
    match bar {
        BarKind::Virgula | BarKind::VirgulaLedger => "virgula",
        BarKind::DivisioMinimis | BarKind::DivisioMinimaLedger => "caesura",
        BarKind::DivisioMinima => "minima",
        BarKind::DivisioMinor | BarKind::Dominican(_) => "minor",
        BarKind::DivisioMaior | BarKind::DivisioMaiorDot => "maior",
        BarKind::DivisioFinalis => "finalis",
    }
}

/// `@form` de `<divLine>` → barre GABC, en l'absence du jeton atomique.
pub fn form_bar(form: &str) -> BarKind {
    match form {
        "virgula" => BarKind::Virgula,
        "caesura" => BarKind::DivisioMinimis,
        "minor" => BarKind::DivisioMinor,
        "maior" => BarKind::DivisioMaior,
        "finalis" => BarKind::DivisioFinalis,
        _ => BarKind::DivisioMinima,
    }
}

/// Famille de signe GABC → élément modificateur MEI 5.1, quand il existe.
///
/// `Guidelines` §6.4.1 : `liquescent`, `oriscus`, `quilisma` et `strophicus`
/// sont des enfants de `<nc>` ; `episema` est un élément de neume.
pub fn component_modifier(kind: SignKind) -> Option<&'static str> {
    match kind {
        SignKind::Liquescentia => Some("liquescent"),
        SignKind::Oriscus => Some("oriscus"),
        SignKind::Quilisma => Some("quilisma"),
        SignKind::Stropha => Some("strophicus"),
        _ => None,
    }
}

/// `@form` de `<episema>` pour un signe GABC, quand MEI 5.1 le porte.
///
/// `att.episema.vis/@form` est `h` (horizontal) ou `v` (vertical),
/// `Guidelines` §6.4.4. Le punctum mora `.` n'a pas d'élément MEI dédié.
pub fn episema_form(kind: SignKind) -> Option<&'static str> {
    match kind {
        SignKind::Ictus => Some("v"),
        SignKind::HorizontalEpisema => Some("h"),
        _ => None,
    }
}

/// Indication de forme GABC → `@tilt` de `<nc>` (`att.ncForm`, §6.4.2).
///
/// Les chiffres d'un punctum inclinatum (`G0`, `G1`, `G2`, `Gabc.tex:454-468`)
/// et les virges ont une direction de trait en MEI (`data.COMPASSDIRECTION`).
/// Les modificateurs numériques qui ne se réduisent pas à une direction
/// restent portés par l'annotation atomique.
pub fn glyph_tilt(glyph: &str) -> Option<&'static str> {
    if glyph.contains('0') {
        Some("sw")
    } else if glyph.contains('1') {
        Some("ne")
    } else if glyph.contains('2') {
        Some("n")
    } else if glyph.contains('V') {
        Some("nw")
    } else if glyph.contains('v') {
        Some("ne")
    } else {
        None
    }
}

/// Indication de forme GABC → `@curve` de `<nc>` (`att.curvatureDirection`,
/// §6.4.2) : `c` pour une courbe horaire, `a` pour une courbe anti-horaire.
pub fn glyph_curve(glyph: &str) -> Option<&'static str> {
    if glyph.contains('<') {
        Some("a")
    } else if glyph.contains('>') {
        Some("c")
    } else {
        None
    }
}

/// Nom stable d'une famille de signe, écrit dans le second jeton de
/// `@type="gabc-note-sign <nom>"` pour permettre la lecture sans dépendance
/// sur le lecteur GABC.
pub fn sign_kind_name(kind: SignKind) -> &'static str {
    match kind {
        SignKind::PunctumMora => "PunctumMora",
        SignKind::Ictus => "Ictus",
        SignKind::HorizontalEpisema => "HorizontalEpisema",
        SignKind::Oriscus => "Oriscus",
        SignKind::Quilisma => "Quilisma",
        SignKind::Virga => "Virga",
        SignKind::Stropha => "Stropha",
        SignKind::Liquescentia => "Liquescentia",
        SignKind::Linea => "Linea",
        SignKind::Cavum => "Cavum",
        SignKind::Accent => "Accent",
        SignKind::Alteration => "Alteration",
        SignKind::Other => "Other",
    }
}

/// Réciproque de [`sign_kind_name`] ; un nom inconnu reste `Other`, le jeton
/// exact du signe étant porté par le texte de l'annotation.
pub fn sign_kind_from_name(name: &str) -> SignKind {
    match name {
        "PunctumMora" => SignKind::PunctumMora,
        "Ictus" => SignKind::Ictus,
        "HorizontalEpisema" => SignKind::HorizontalEpisema,
        "Oriscus" => SignKind::Oriscus,
        "Quilisma" => SignKind::Quilisma,
        "Virga" => SignKind::Virga,
        "Stropha" => SignKind::Stropha,
        "Liquescentia" => SignKind::Liquescentia,
        "Linea" => SignKind::Linea,
        "Cavum" => SignKind::Cavum,
        "Accent" => SignKind::Accent,
        "Alteration" => SignKind::Alteration,
        _ => SignKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clefs_round_trip_through_shape_and_line() {
        for token in ["c1", "c2", "c3", "c4", "f3", "f4"] {
            let parts = clef_parts(token).expect("a GABC clef");
            let rebuilt = clef_token(&parts.shape.to_lowercase().to_string(), parts.line);
            assert_eq!(rebuilt, token, "the atomic token is kept separately");
        }
        // Le `b` de `cb3` (clé basse) et le `@` d'une clé double n'ont pas de
        // porteur MEI : le jeton exact reste dans l'annotation atomique.
        let parts = clef_parts("cb3").expect("a low C clef");
        assert_eq!(
            clef_token(&parts.shape.to_lowercase().to_string(), parts.line),
            "c3"
        );
        assert_eq!(
            clef_parts("c1@c4"),
            Some(MeiClef {
                shape: 'C',
                line: 1
            })
        );
    }

    #[test]
    fn pitch_positions_follow_gregorios_clef_key() {
        // `c3` : clé de do sur la troisième ligne, clé = 5. `g` est à la
        // hauteur 9, donc au degré -1 = si sous le do moyen.
        assert_eq!(nc_position('g', "c3"), ('b', 3));
        assert_eq!(nc_letter(Some("b"), Some(3), "c3"), Some('g'));
        assert_eq!(nc_position('g', "f4"), ('c', 4));
        assert_eq!(nc_letter(Some("c"), Some(4), "f4"), Some('g'));
    }

    #[test]
    fn neume_modifiers_cover_the_gabc_shapes_with_an_mei_carrier() {
        assert_eq!(
            component_modifier(SignKind::Liquescentia),
            Some("liquescent")
        );
        assert_eq!(component_modifier(SignKind::Oriscus), Some("oriscus"));
        assert_eq!(component_modifier(SignKind::Quilisma), Some("quilisma"));
        assert_eq!(component_modifier(SignKind::Stropha), Some("strophicus"));
        assert_eq!(episema_form(SignKind::Ictus), Some("v"));
        assert_eq!(episema_form(SignKind::HorizontalEpisema), Some("h"));
        assert_eq!(episema_form(SignKind::PunctumMora), None);
        assert_eq!(glyph_tilt("G0"), Some("sw"));
        assert_eq!(glyph_curve("~<"), Some("a"));
    }

    #[test]
    fn division_forms_cover_all_gabc_bars() {
        for bar in [
            BarKind::Virgula,
            BarKind::DivisioMinimis,
            BarKind::DivisioMinima,
            BarKind::DivisioMinor,
            BarKind::DivisioMaior,
            BarKind::DivisioFinalis,
            BarKind::Dominican(3),
        ] {
            let form = bar_form(bar);
            assert!(!form.is_empty());
        }
        assert_eq!(form_bar("finalis"), BarKind::DivisioFinalis);
        assert_eq!(form_bar("minima"), BarKind::DivisioMinima);
    }
}
