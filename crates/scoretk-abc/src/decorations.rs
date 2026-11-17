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

use mscz2xml::model::{ArticulationType, HairpinType, OrnamentType};

/// Liste des nuances reconnues par ABC et par MuseScore.
const DYNAMICS: [&str; 20] = [
    "pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "ffff", "fp", "pf", "sf", "sfz", "sfp",
    "fz", "rf", "rfz", "sfpp", "niente",
];

/// Vrai si un texte est une nuance standard.
pub(crate) fn is_dynamic(text: &str) -> bool {
    let lowered = text.trim().to_ascii_lowercase();
    DYNAMICS.contains(&lowered.as_str())
}

/// Décoration produite par la lecture d'un `!nom!`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Decoration {
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

/// Nom ABC d'une articulation, ou `None` si elle n'a pas d'équivalent.
pub(crate) fn articulation_token(kind: &ArticulationType) -> Option<&'static str> {
    let token = match kind {
        ArticulationType::Staccato => "staccato",
        ArticulationType::Staccatissimo | ArticulationType::Spiccato => "staccatissimo",
        ArticulationType::Accent | ArticulationType::SoftAccent => "accent",
        ArticulationType::Marcato => "marcato",
        ArticulationType::Tenuto => "tenuto",
        ArticulationType::Portato => "portato",
        ArticulationType::Fermata => "fermata",
        ArticulationType::FermataShort
        | ArticulationType::FermataLong
        | ArticulationType::FermataVeryShort
        | ArticulationType::FermataVeryLong => "fermata",
        ArticulationType::BreathMark => "breath",
        ArticulationType::Caesura => "caesura",
        ArticulationType::UpBow => "upbow",
        ArticulationType::DownBow => "downbow",
        ArticulationType::Harmonic => "harmonic",
        ArticulationType::Open => "open",
        ArticulationType::Stopped => "stopped",
        ArticulationType::SnapPizzicato => "snap",
        ArticulationType::SoftAccentStaccato
        | ArticulationType::SoftAccentTenuto
        | ArticulationType::SoftAccentTenutoStaccato => "accent",
        ArticulationType::Stress => "emphasis",
        ArticulationType::Unstress | ArticulationType::Other(_) => return None,
    };
    Some(token)
}

/// Nom ABC d'un ornement, ou `None` s'il n'a pas d'équivalent.
pub(crate) fn ornament_token(kind: &OrnamentType) -> Option<&'static str> {
    let token = match kind {
        OrnamentType::TrillMark => "trill",
        OrnamentType::Turn | OrnamentType::TurnSlash => "turn",
        OrnamentType::InvertedTurn => "invertedturn",
        OrnamentType::Mordent | OrnamentType::MordentLong => "mordent",
        OrnamentType::InvertedMordent | OrnamentType::InvertedMordentLong => "invertedmordent",
        OrnamentType::Haydn => "haydn",
        OrnamentType::Schleifer => "schleifer",
        _ => return None,
    };
    Some(token)
}

/// Décoration lue dans un `!nom!`.
pub(crate) fn parse_decoration(name: &str) -> Option<Decoration> {
    let lowered = name.trim().to_ascii_lowercase();
    if DYNAMICS.contains(&lowered.as_str()) {
        return Some(Decoration::Dynamic(lowered));
    }
    let decoration = match lowered.as_str() {
        "staccato" | "." => Decoration::Articulation(ArticulationType::Staccato),
        "staccatissimo" => Decoration::Articulation(ArticulationType::Staccatissimo),
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
        "invertedmordent" | "p" => Decoration::Ornament(OrnamentType::InvertedMordent),
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
        assert_eq!(
            articulation_token(&ArticulationType::Staccato),
            Some("staccato")
        );
        assert_eq!(ornament_token(&OrnamentType::TrillMark), Some("trill"));
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
