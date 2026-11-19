// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Les noms de décorations sont ceux de la norme ABC 2.1
// (<https://abcnotation.com/wiki/abc:standard:v2.1>). Les correspondances
// d'articulations et d'ornements reprennent le comportement de `xml2abc.py`
// et `abc2xml.py` de Willem Vree (LGPL) ; aucune ligne n'est copiée.
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

//! Décorations ABC : nuances, articulations, ornements et soufflets.
//!
//! Une décoration s'écrit `!nom!` avant la note qu'elle affecte. Les
//! abréviations d'une lettre (`.`, `u`, `v`, `H`, `M`, `P`, `T`) sont
//! reconnues à la lecture, mais l'écriture emploie toujours la forme longue,
//! plus lisible et sans ambiguïté avec les hauteurs.

use scoretk_core::model::{ArticulationType, HairpinType, OrnamentType};

/// Liste des nuances reconnues par ABC et par MuseScore.
const DYNAMICS: [&str; 20] = [
    "pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "ffff", "fp", "pf", "sf", "sfz", "sfp",
    "fz", "rf", "rfz", "sfpp", "niente",
];

/// Vrai si un texte est une nuance standard.
pub fn is_dynamic(text: &str) -> bool {
    let lowered = text.trim().to_ascii_lowercase();
    DYNAMICS.contains(&lowered.as_str())
}

/// Décoration produite par la lecture d'un `!nom!`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoration {
    /// Nuance (`!p!`, `!mf!`…).
    Dynamic(String),
    /// Articulation.
    Articulation(ArticulationType),
    /// Ornement.
    Ornament(OrnamentType),
    /// Début de soufflet (`!<(!`, `!>(!`).
    HairpinStart(HairpinType),
    /// Fin de soufflet (`!<)!`, `!>)!`, `!diminuendo!`…).
    HairpinEnd,
}

/// Jeton ABC complet d'une articulation, ou `None` sans équivalent.
///
/// Port de `xml2abc.py::note_ornamentation_map` (lignes 32-57) : ce sont les
/// jetons qu'écrit le script de référence. En particulier `staccato` → `.`,
/// `staccatissimo`/`spiccato` → `!wedge!`, `accent` → `!>!` et
/// `strong-accent` → `!^!`. Les anciens noms longs (`!staccato!`,
/// `!marcato!`, `!accent!`, `!staccatissimo!`) sont ignorés par `abc2midi`
/// et produisaient un avertissement par note.
pub fn articulation_token(kind: &ArticulationType) -> Option<&'static str> {
    let token = match kind {
        ArticulationType::Staccato => ".",
        ArticulationType::Staccatissimo | ArticulationType::Spiccato => "!wedge!",
        ArticulationType::Accent | ArticulationType::SoftAccent => "!>!",
        ArticulationType::Marcato => "!^!",
        ArticulationType::Tenuto => "!tenuto!",
        ArticulationType::Portato => "!tenuto!.",
        ArticulationType::Fermata => "!fermata!",
        ArticulationType::FermataShort
        | ArticulationType::FermataLong
        | ArticulationType::FermataVeryShort
        | ArticulationType::FermataVeryLong => "!fermata!",
        ArticulationType::BreathMark => "!breath!",
        ArticulationType::Caesura => "!caesura!",
        ArticulationType::UpBow => "u",
        ArticulationType::DownBow => "v",
        ArticulationType::Harmonic | ArticulationType::Open => "!open!",
        ArticulationType::Stopped => "!plus!",
        ArticulationType::SnapPizzicato => "!snap!",
        ArticulationType::SoftAccentStaccato
        | ArticulationType::SoftAccentTenuto
        | ArticulationType::SoftAccentTenutoStaccato => "!>!",
        ArticulationType::Stress => "!>!",
        ArticulationType::Unstress | ArticulationType::Other(_) => return None,
    };
    Some(token)
}

/// Vrai si la nuance est dans `xml2abc.py::dynamics_map` (lignes 59-71) et
/// doit donc être écrite comme `!nom!`. Les autres (`sf`, `fz`, `fp`…)
/// sont des annotations texte pour cet outil.
pub fn is_dynamic_token(text: &str) -> bool {
    const MAPPED: [&str; 11] = [
        "pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "ffff", "sfz",
    ];
    MAPPED.contains(&text.trim().to_ascii_lowercase().as_str())
}

/// Nom ABC d'un ornement, ou `None` s'il n'a pas d'équivalent.
pub fn ornament_token(kind: &OrnamentType) -> Option<&'static str> {
    let token = match kind {
        OrnamentType::TrillMark => "trill",
        OrnamentType::Turn | OrnamentType::TurnSlash => "turn",
        OrnamentType::InvertedTurn => "invertedturn",
        OrnamentType::Mordent | OrnamentType::MordentLong => "mordent",
        // § 4.14 de la norme ABC 2.1 : `!mordent!` = `!lowermordent!` et
        // `!uppermordent!` (abréviation `P`) sont définis, mais pas
        // `!invertedmordent!`, qu'un lecteur conforme ignore. `xml2abc.py`
        // (`note_ornamentation_map`, lignes 34-35) mappe précisément
        // `mordent` sur `M` et `inverted-mordent` sur `P`.
        OrnamentType::InvertedMordent | OrnamentType::InvertedMordentLong => "uppermordent",
        OrnamentType::Haydn => "haydn",
        OrnamentType::Schleifer => "schleifer",
        _ => return None,
    };
    Some(token)
}

/// Décoration lue dans un `!nom!`.
pub fn parse_decoration(name: &str) -> Option<Decoration> {
    let lowered = name.trim().to_ascii_lowercase();
    if DYNAMICS.contains(&lowered.as_str()) {
        return Some(Decoration::Dynamic(lowered));
    }
    let decoration = match lowered.as_str() {
        "staccato" | "." => Decoration::Articulation(ArticulationType::Staccato),
        "staccatissimo" | "wedge" => Decoration::Articulation(ArticulationType::Staccatissimo),
        "accent" | ">" => Decoration::Articulation(ArticulationType::Accent),
        "marcato" | "^" => Decoration::Articulation(ArticulationType::Marcato),
        "tenuto" | "-" => Decoration::Articulation(ArticulationType::Tenuto),
        "portato" => Decoration::Articulation(ArticulationType::Portato),
        "fermata" | "fermataabove" | "fermatabelow" => {
            Decoration::Articulation(ArticulationType::Fermata)
        }
        "breath" | "breathmark" => Decoration::Articulation(ArticulationType::BreathMark),
        "caesura" => Decoration::Articulation(ArticulationType::Caesura),
        "upbow" | "u" => Decoration::Articulation(ArticulationType::UpBow),
        "downbow" | "v" => Decoration::Articulation(ArticulationType::DownBow),
        "harmonic" | "flageolet" => Decoration::Articulation(ArticulationType::Harmonic),
        "open" => Decoration::Articulation(ArticulationType::Open),
        "stopped" | "plus" => Decoration::Articulation(ArticulationType::Stopped),
        "snap" => Decoration::Articulation(ArticulationType::SnapPizzicato),
        "emphasis" => Decoration::Articulation(ArticulationType::Stress),
        "trill" | "t" => Decoration::Ornament(OrnamentType::TrillMark),
        "mordent" | "m" => Decoration::Ornament(OrnamentType::Mordent),
        // § 4.14 : `!uppermordent!` est le nom long de l'abréviation `P`,
        // accepté aussi bien à la lecture qu'à l'écriture.
        "invertedmordent" | "uppermordent" | "p" => {
            Decoration::Ornament(OrnamentType::InvertedMordent)
        }
        "turn" => Decoration::Ornament(OrnamentType::Turn),
        "invertedturn" => Decoration::Ornament(OrnamentType::InvertedTurn),
        "haydn" => Decoration::Ornament(OrnamentType::Haydn),
        "schleifer" => Decoration::Ornament(OrnamentType::Schleifer),
        "<(" => Decoration::HairpinStart(HairpinType::Crescendo),
        "<)" => Decoration::HairpinEnd,
        ">(" => Decoration::HairpinStart(HairpinType::Diminuendo),
        ">)" | "diminuendo" => Decoration::HairpinEnd,
        "crescendo" => Decoration::HairpinStart(HairpinType::Crescendo),
        _ => return None,
    };
    Some(decoration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamiques() {
        assert!(is_dynamic("p"));
        assert!(is_dynamic("sfz"));
        assert!(!is_dynamic("Allegro"));
        assert_eq!(
            parse_decoration("mf"),
            Some(Decoration::Dynamic("mf".to_owned()))
        );
    }

    #[test]
    fn articulations_et_ornements() {
        assert_eq!(articulation_token(&ArticulationType::Staccato), Some("."));
        assert_eq!(
            articulation_token(&ArticulationType::Staccatissimo),
            Some("!wedge!")
        );
        assert_eq!(ornament_token(&OrnamentType::TrillMark), Some("trill"));
        assert!(is_dynamic_token("sfz"));
        assert!(!is_dynamic_token("sf"));
        assert_eq!(
            parse_decoration("staccato"),
            Some(Decoration::Articulation(ArticulationType::Staccato))
        );
        assert_eq!(
            parse_decoration("trill"),
            Some(Decoration::Ornament(OrnamentType::TrillMark))
        );
        assert!(parse_decoration("inconnue").is_none());
    }

    #[test]
    fn soufflets() {
        assert_eq!(
            parse_decoration("<("),
            Some(Decoration::HairpinStart(HairpinType::Crescendo))
        );
        assert_eq!(parse_decoration(">)"), Some(Decoration::HairpinEnd));
    }
}
